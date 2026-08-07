# PRP — P1.M7.T2.S2: Add `.rpm` CI job to `release.yml`

---

## Goal

**Feature Goal**: Add a post-publish, tag-only `rpm` job to
`.github/workflows/release.yml` that builds the QMKonnect `.rpm` on a Fedora
container (via `cargo-generate-rpm`, **no** `-lhidapi-hidraw`) and appends
`qmkonnect-<ver>-linux-x86_64.rpm` to the GitHub Release created by the `publish`
job — completing the native-`.rpm` distribution channel (PRD F15; `PACKAGING.md`
§4.4 recipe + §9 CI contract).

**Deliverable**: A single new `rpm:` job block inserted into
`.github/workflows/release.yml` (immediately after the existing `deb:` job),
comprising a Mode-A documentation comment + 9 ordered steps (dnf deps → checkout
→ rust toolchain → version → install cargo-generate-rpm → cargo build --release
→ cargo generate-rpm → rename → gh release upload). **This task edits ONLY
`release.yml`.** It consumes (does not author) the `[package.metadata.generate-rpm]`
block + `packaging/rpm/{postin,postun}` produced by the parallel sibling
**P1.M7.T2.S1** (see its PRP — treated as a CONTRACT here).

**Success Definition**: On a `v*` tag push, after `publish` succeeds, the `rpm`
job runs on `fedora:latest`, builds `qmkonnect-<ver>-linux-x86_64.rpm`, and
attaches it to the Release (idempotent via `gh release upload --clobber`). On a
`workflow_dispatch` dry-run the job is skipped (`if: github.event_name == 'push'`).
A `.rpm` build failure never blocks the core release (it is NOT in
`publish.needs`). The job sets no `-lhidapi-hidraw` flag (Fedora unified hidapi).

## User Persona

**Target User**: Fedora / RHEL / Rocky / Alma / openSUSE end users (the F15
`.rpm` audience) and the maintainer cutting releases.

**Use Case**: A Fedora user runs `sudo dnf install
<release-url>/qmkonnect-<ver>-linux-x86_64.rpm` to install QMKonnect with native
dependency resolution + install/uninstall hooks, instead of the generic tarball.

**User Journey**: maintainer pushes `v0.2.8` tag → CI builds every platform →
`publish` creates the GitHub Release → the `rpm` job (this task) + `deb` job
append their native packages → the Fedora user `dnf install`s the `.rpm` asset.

**Pain Points Addressed**: Fedora-family users previously had only the generic
tarball (manual `install.sh`); the `.rpm` gives them `dnf install` with proper
`Requires` resolution + the `%post`/`%postun` lifecycle the Arch/`.deb` channels
already have.

## Why

- **F15 (community package-manager distribution)** calls for native `.rpm`
  alongside `.deb` (`PACKAGING.md` §6, F15 row). The `.deb` CI job already landed
  (P1.M7.T1.S2). The `.rpm` build recipe (`[package.metadata.generate-rpm]` +
  `packaging/rpm/*`) is produced by the parallel sibling **P1.M7.T2.S1**; this
  task is the CI wiring that actually produces a release `.rpm` artifact.
- **Scope boundary**: this task edits **only** `.github/workflows/release.yml`
  (one new job). It does NOT touch `Cargo.toml`, `packaging/rpm/*`, `spec/*`, or
  any `.rs`. The build recipe + maintainer scripts are P1.M7.T2.S1 (consumed as
  a contract). The `.deb` job (P1.M7.T1.S2) is the structural template to mirror.
- **Coexistence with siblings**: the `rpm` job is a POST-publish appender (like
  `deb`), kept OFF `publish.needs` so a `.rpm` build failure never blocks the
  core release. It does not conflict with the `arch`/`aur` jobs (different
  distro family).

## What

User-/maintainer-visible behavior:
- Push a `v*` tag → `publish` creates the Release → the new `rpm` job runs on
  `fedora:latest` inside an `ubuntu-latest` runner, installs the Fedora build-dep
  set, builds the release binary (no hidraw flag), runs `cargo generate-rpm`,
  renames `target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm` →
  `qmkonnect-<ver>-linux-x86_64.rpm`, and uploads it to the Release with
  `gh release upload --clobber` (idempotent on re-runs).
- `workflow_dispatch` dry-runs skip the job entirely (no Release to append to).

### Success Criteria

- [ ] A new `rpm:` job exists in `.github/workflows/release.yml`, placed
      immediately after the `deb:` job.
- [ ] `runs-on: ubuntu-latest` + `container: fedora:latest` (NO native
      `fedora-latest` runner exists; mirrors the `arch` job's `container:`
      pattern).
- [ ] `needs: [publish]` + `if: github.event_name == 'push'` (tag-only,
      post-publish — identical to `deb`).
- [ ] `permissions: contents: write` (so `gh release upload` can append).
- [ ] dnf step installs the **full** Fedora `-devel` build set (NOT just the
      spec §9 3-pack — see Context §"Spec-vs-reality").
- [ ] Rust toolchain via `dtolnay/rust-toolchain@stable` (NOT `dnf install rust`
      — Fedora rust < MSRV 1.88).
- [ ] `cargo install --locked cargo-generate-rpm` then `cargo build --release`
      then `cargo generate-rpm`, with **no** `-lhidapi-hidraw` anywhere.
- [ ] Rename `qmkonnect-<ver>-1.x86_64.rpm` → `qmkonnect-<ver>-linux-x86_64.rpm`.
- [ ] `gh release upload "${GITHUB_REF_NAME}" "qmkonnect-<ver>-linux-x86_64.rpm"
      --clobber`.
- [ ] `rpm` is NOT added to `publish.needs`.
- [ ] Mode-A comment block documents the job (Fedora target, post-publish,
      unified-hidapi no-flag rationale, full-build-dep rationale, MSRV→toolchain
      rationale, cross-ref to P1.M7.T2.S1 + spec §4.4/§9).

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?" — **YES.** The exact YAML job block is pinned
> in Task 1 (copy-paste ready); the proven analogs (`deb` job for post-publish
> structure, `arch` job for `container:` + deps-before-checkout pattern) are in
> the SAME workflow file; the build-dep translation + every Fedora gotcha
> (appindicator package name, MSRV→toolchain, no-native-runner, gh-in-container)
> are enumerated below with reasoning.

### Spec-vs-reality (READ FIRST — the #1 one-pass failure mode)

`spec/PACKAGING.md §9` and the work-item contract summarize the dnf step as
**3 packages**: `hidapi-devel libxdo-devel pkg-config`. **That list cannot build
the binary.** It is only the *packaging-level* dependency intent. The default
`cargo build --release` (Cargo.toml `[features] default = ["…", "linux-tray"]`,
where `linux-tray = ["dep:ksni", "dep:gtk"]`) compiles gtk-rs + the wayland/gnome/
atspi backends + tao/tray-icon/libxdo/hidapi → needs the **full** GTK3/X11/systemd
`-devel` stack. The **`.deb` job proves this**: spec §9 *also* lists only 3 for
`.deb`, but the real `deb` job installs the full apt set
(`build-essential libgtk-3-dev libglib2.0-dev libayatana-appindicator3-dev
libx11-dev libxcb1-dev libxdo-dev libhidapi-dev libudev-dev`). **The `.rpm` job
must do the Fedora equivalent.** Installing the full superset satisfies the spec's
3-pack subset. See `research/fedora-build-deps.md` for the verified translation.

### Documentation & References

```yaml
# MUST READ — the work item being wired (consumed as a CONTRACT)
- file: plan/007_fb356ba503b4/P1M7T2S1/PRP.md
  why: Defines the [package.metadata.generate-rpm] block + packaging/rpm/{postin,
       postun,README.md} this CI job consumes. Pins the cargo-generate-rpm output
       path target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm and the no-hidraw-flag
       invariant. Treat it as already-implemented.
  critical: The output filename carries the RPM release number "-1" (from
            release="1") and arch x86_64; the release-asset rename drops the -1
            → qmkonnect-<ver>-linux-x86_64.rpm.

# MUST READ — the authoritative CI contract (spec is read-only)
- file: spec/PACKAGING.md
  section: "§9 CI Release → the .rpm job (NEW) bullet"
  why: Pins the job intent: Fedora, dnf deps, cargo install cargo-generate-rpm,
       cargo generate-rpm (no -lhidapi-hidraw), rename to
       qmkonnect-<ver>-linux-x86_64.rpm, upload.
  critical: §9's 3-pack dnf list is a SUMMARY — the real job needs the full -devel
            set (see Spec-vs-reality + research/fedora-build-deps.md).
- file: spec/PACKAGING.md
  section: "§4.4 .rpm via cargo-generate-rpm"
  why: Pins the recipe (target Fedora/RHEL/Rocky/Alma/openSUSE), the no-hidraw
       build invariant (Fedora unified hidapi, §2), the output path + release
       rename, and the openSUSE shared-spec note.
  critical: Build on Fedora covers glibc 2.34+ (RHEL 9 family). The §4.4
            "Build-deps (CI dnf step): hidapi-devel libxdo-devel pkgconfig" line
            is the same incomplete summary as §9 — use the full set from Task 1.

# MUST READ — the structural template (post-publish, tag-only, gh upload)
- file: .github/workflows/release.yml
  section: "the `deb:` job (P1.M7.T1.S2 — already in tree)"
  why: The rpm job is a 1:1 structural mirror: needs:[publish],
       if: github.event_name == 'push', permissions: contents: write, the
       version-from-cargo-metadata step, the rename step, and the
       `gh release upload "${GITHUB_REF_NAME}" … --clobber` upload step.
  pattern: |
    deb:
      needs: [publish]
      if: github.event_name == 'push'
      runs-on: ubuntu-22.04           # ← rpm uses ubuntu-latest + container: fedora:latest
      permissions: { contents: write }
      steps:
        - checkout → rust-toolchain → install-deps → ver → cargo install cargo-deb
          → cargo deb → rename → gh release upload --clobber

# MUST READ — the container pattern (deps-before-checkout, no git in image)
- file: .github/workflows/release.yml
  section: "the `arch:` job"
  why: The ONLY existing container-based job in this workflow. Proves that
       `runs-on: ubuntu-latest` + `container: <img>:latest` works here, and that
       the FIRST step MUST `dnf`/`pacman install git` BEFORE actions/checkout
       (the container image ships no git). The rpm job copies this skeleton.
  critical: arch gets rust from pacman; rpm MUST get rust from
            dtolnay/rust-toolchain@stable instead (Fedora dnf rust < MSRV 1.88 —
            see Gotcha #2).

# MUST READ — the publish job (this job's dependency + the action it appends to)
- file: .github/workflows/release.yml
  section: "the `publish:` job"
  why: publish creates the GitHub Release (softprops/action-gh-release@v2) and is
       the dependency of every post-publish appender (deb, aur, homebrew, scoop,
       winget, asdf, AND now rpm). gh release upload appends to the Release
       publish created. rpm is NOT added to publish.needs.

# EXTERNAL — cargo-generate-rpm does NOT auto-build
- url: https://github.com/cat-in-136/cargo-generate-rpm#usage
  why: Confirms `cargo generate-rpm` reads target/release/* but does NOT run
       cargo build — you MUST `cargo build --release` first (Task 1 step order).
  critical: Step order is build-then-generate-rpm (not the reverse).

# EXTERNAL — GitHub CLI installable on Fedora via dnf (for the upload step)
- url: https://github.com/cli/cli/blob/trunk/docs/install_linux.md#fedora-centos-red-hat-enterprise-linux-dnf
  why: Confirms `sudo dnf install gh` works on Fedora (gh is in the main repo) so
       the `gh release upload … --clobber` upload step (mirror of the deb job)
       works inside the fedora container.
  critical: Fallback if dnf gh ever fails: use softprops/action-gh-release@v2
            (already vetted in the publish job) — see research note §5.

# RESEARCH NOTES (this task)
- docfile: plan/007_fb356ba503b4/P1M7T2S2/research/fedora-build-deps.md
  why: Verified Fedora -devel translation of the deb apt set + the
       libayatana→libappindicator-gtk3 gotcha (Ayatana not in Fedora main repos).
- docfile: plan/007_fb356ba503b4/P1M7T2S2/research/container-ci-patterns.md
  why: no-native-fedora-runner, MSRV→dtolnay-toolchain, omit-rust-cache, step
       ordering, gh-in-container, dependency wiring, placement.
```

### Current Codebase Tree (relevant slice)

```bash
.github/workflows/release.yml   # EDIT — insert the `rpm:` job after the `deb:` job
Cargo.toml                      # READ-ONLY here; [package.metadata.generate-rpm] authored by P1.M7.T2.S1
packaging/rpm/{postin,postun,README.md}  # READ-ONLY here; authored by P1.M7.T2.S1
spec/PACKAGING.md               # READ-ONLY source of truth (§4.4 recipe, §9 CI contract)
```

### Desired Codebase Tree (files this task touches)

```bash
.github/workflows/release.yml   # EDIT ONLY — one new `rpm:` job block (after `deb:`)
# (no other file is created or modified by this task)
```

### Known Gotchas of our codebase & library quirks

```yaml
# CRITICAL (one-pass #1): the spec §9 / contract 3-pack dnf list
# (hidapi-devel libxdo-devel pkg-config) CANNOT build the binary. The default
# build compiles gtk-rs + the wayland/gnome/atspi/hyprland backends + tao/tray-
# icon/libxdo/hidapi and needs the FULL Fedora -devel set (gtk3-devel,
# glib2-devel, libappindicator-gtk3-devel, libX11-devel, libxcb-devel, libxdo-
# devel, hidapi-devel, systemd-devel, gcc). The .deb job proves this (it installs
# the full apt set, not the spec's 3). => Use the full dnf line from Task 1.

# CRITICAL (one-pass #2): there is NO native `fedora-latest` GitHub Actions
# runner. MUST use runs-on: ubuntu-latest + container: fedora:latest (mirror the
# arch job's container: archlinux:latest). A bare `runs-on: fedora-latest` is
# rejected by GitHub ("no runner registered").

# CRITICAL (one-pass #3): do NOT get Rust from `dnf install rust`. Fedora's rust
# is ~1.79–1.85 (< MSRV 1.88 in Cargo.toml) → build fails the rust-version check.
# Use dtolnay/rust-toolchain@stable (mirror the deb job) — always ≥1.88. The dnf
# step installs the C/GTK toolchain + git/jq/gh ONLY.

# CRITICAL (one-pass #4): Fedora has NO libayatana-appindicator in main repos
# (Red Hat Bugzilla #2253582 is a pending Review Request). Use
# libappindicator-gtk3-devel (legacy, in main Fedora, provides appindicator3-0.1
# pkg-config). NEVER libayatana-appindicator* on Fedora.

# CRITICAL (one-pass #5): the fedora container ships NO git. The dnf step MUST
# install git BEFORE actions/checkout@v4 (checkout needs git). Mirror the arch
# job's "Install build dependencies" first-step pattern.

# CRITICAL (build invariance): NO -lhidapi-hidraw. Fedora/RHEL ship a UNIFIED
# hidapi (≥0.14) that auto-selects hidraw at runtime (spec §2). The hidraw flag
# is Arch-only (the arch PKGBUILD). Plain `cargo build --release` is correct.

# CRITICAL (cargo generate-rpm does NOT auto-build): run `cargo build --release`
# FIRST, then `cargo generate-rpm`. Reversing the order packages a stale/missing
# target/release/{qmkonnect,qmkonnect-hid-id} → empty/failed .rpm.

# GOTCHA (release number in filename): cargo-generate-rpm emits
# target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm (the "-1" is the RPM Release
# from release="1" in [package.metadata.generate-rpm]). The release ASSET name
# drops the -1: qmkonnect-<ver>-linux-x86_64.rpm. Match the source mv exactly.

# GOTCHA (gh not in the fedora image): gh (GitHub CLI) is preinstalled on ubuntu
# runners but NOT in fedora:latest. `dnf install gh` (gh is in Fedora's main
# repo) puts it in the container so `gh release upload --clobber` works. Fallback
# if dnf gh ever fails: softprops/action-gh-release@v2 (vetted in the publish
# job) — see research/container-ci-patterns.md §5.

# GOTCHA (rust-cache in a container): OMIT Swatinem/rust-cache@v2. The only
# existing container job (arch) deliberately omits it (UID/cache-permission
# friction). A release tag is infrequent; build-from-scratch is fine. (Optional
# optimization documented — add only after verifying cache restore as root.)

# GOTCHA (jq): the version step uses `cargo metadata … | jq …` (mirror the deb
# job). jq is NOT in fedora:latest by default → it's in the dnf install line.

# GOTCHA (--locked): `cargo install --locked cargo-generate-rpm` (mirror the deb
# job's `cargo install --locked cargo-deb`). If --locked ever fails (crate
# shipped no Cargo.lock), drop --locked — but keep it for reproducibility first.
```

## Implementation Blueprint

### Data models and structure

_None._ This is a CI workflow edit — a single YAML job block. No Rust types,
Cargo metadata, or runtime models change. The only file touched is
`.github/workflows/release.yml` (pure append of one job after the `deb:` job).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT .github/workflows/release.yml — INSERT the `rpm:` job block
        IMMEDIATELY AFTER the `deb:` job (before the `aur:` job).
  - FIND the END of the `deb:` job. It ends with its "Upload .deb to the GitHub
        Release" step (the `gh release upload "${GITHUB_REF_NAME}"
        "qmkonnect-${VERSION}-linux-amd64.deb" --clobber` step). Insert the new
        `rpm:` job right after that step's last line, BEFORE the `  # ─────…`
        comment that opens the `aur:` job.
  - INSERT exactly the block in "Task 1 Block" below (copy-paste ready). Keep the
        2-space YAML indentation consistent with the sibling jobs (job key at
        column 0, `name:`/`needs:`/… at column 2, step keys at column 4).
  - PRESERVE: every existing job (macos, windows, linux-binary, arch, gnome-
        extension, nix, publish, deb, aur, homebrew-tap, scoop-bucket, winget,
        asdf-plugin), the top-level `on:`/`env:`/`permissions:`/`jobs:` keys, and
        the `publish.needs:` list (do NOT add `rpm` to it).
  - DEPENDENCIES: the `[package.metadata.generate-rpm]` block + packaging/rpm/
        {postin,postun} from P1.M7.T2.S1 must exist in the tree (they do — that
        PRP is the parallel sibling; verify with `grep -n 'generate-rpm' Cargo.toml`
        + `ls packaging/rpm/`). If absent, STOP and flag the dependency — this
        task consumes, it does not author, those files.

  # ── Task 1 Block ───────────────────────────────────────────────────────────
  # Use EXACTLY this. Each `why` is in the comment block; the dnf line is the
  # FULL build-dep set (not the spec §9 3-pack — see Spec-vs-reality).

  # ─────────────────────────────────────────────────────────────────────────
  # .rpm — native Fedora/RHEL/Rocky/Alma/openSUSE package via cargo-generate-rpm
  # (spec/PACKAGING.md §4.4 recipe + §9 CI contract). The
  # [package.metadata.generate-rpm] block + packaging/rpm/{postin,postun} are
  # authored by P1.M7.T2.S1; this job consumes them.
  #
  # A POST-PUBLISH job (mirror of `deb`): runs after `publish` creates the
  # GitHub Release and APPENDS qmkonnect-<ver>-linux-x86_64.rpm to it via
  # `gh release upload --clobber` (idempotent on re-runs). NOT in
  # publish.needs — a .rpm build failure must never block the core release.
  # Skipped on workflow_dispatch dry-runs (publish creates no release).
  #
  # WHY a Fedora CONTAINER (not a native runner): GitHub has NO `fedora-latest`
  # runner. We use runs-on: ubuntu-latest + container: fedora:latest — the SAME
  # pattern the `arch` job uses (container: archlinux:latest). The first step
  # dnf-installs git + the build deps BEFORE actions/checkout (the container
  # image ships no git), mirroring the arch job's deps-first step.
  #
  # WHY the FULL -devel set (not spec §9's 3-pack): the default `cargo build
  # --release` compiles gtk-rs + the wayland/gnome/atspi/hyprland backends +
  # tao/tray-icon/libxdo/hidapi (Cargo.toml [features] default includes
  # linux-tray=[dep:ksni,dep:gtk]). The spec §9 / contract 3-pack
  # (hidapi-devel libxdo-devel pkg-config) is a SUMMARY; the real job needs the
  # full GTK3/X11/systemd stack — exactly as the `deb` job installs the full apt
  # set. Installing the superset satisfies the subset.
  #
  # WHY dtolnay/rust-toolchain@stable (NOT `dnf install rust`): Fedora's packaged
  # rust is ~1.79–1.85, BELOW the Cargo.toml MSRV (rust-version = "1.88"). The
  # action always installs latest stable (≫1.88). The dnf step provides only the
  # C/GTK toolchain + git/jq/gh.
  #
  # WHY libappindicator-gtk3-devel (NOT libayatana-appindicator*): Fedora has NO
  # libayatana-appindicator in its main repos (Red Hat Bugzilla #2253582 is a
  # pending Review Request). libappindicator-gtk3-devel is the legacy package in
  # main Fedora that provides the appindicator3-0.1 pkg-config the GTK linkage
  # resolves against.
  #
  # Build uses NO -lhidapi-hidraw: Fedora/RHEL ship a UNIFIED hidapi (>=0.14)
  # that auto-selects the hidraw backend at runtime (spec §2). The hidraw flag
  # is Arch-only (the arch PKGBUILD). Plain `cargo build --release` is correct.
  # ─────────────────────────────────────────────────────────────────────────
  rpm:
    name: Linux (.rpm via cargo-generate-rpm)
    needs: [publish]
    if: github.event_name == 'push'
    runs-on: ubuntu-latest
    container: fedora:latest
    permissions:
      contents: write
    steps:
      # Container ships no git/toolchain → install the FULL build-dep set FIRST
      # (before checkout). gh + jq needed for later steps; git for checkout.
      - name: Install Fedora build dependencies
        run: |
          dnf install -y \
            gcc pkgconf-pkg-config \
            gtk3-devel glib2-devel libappindicator-gtk3-devel \
            libX11-devel libxcb-devel libxdo-devel \
            hidapi-devel systemd-devel \
            git jq gh ca-certificates

      - uses: actions/checkout@v4

      # NOT `dnf install rust`: Fedora's rust (~1.79–1.85) < MSRV 1.88. The
      # action installs latest stable. (Mirror of the `deb` job.)
      - uses: dtolnay/rust-toolchain@stable

      - name: Determine version
        id: ver
        run: |
          v=$(cargo metadata --no-deps --format-version 1 \
              | jq -r '.packages[] | select(.name=="qmkonnect") | .version')
          echo "version=$v" >> "$GITHUB_OUTPUT"

      - name: Install cargo-generate-rpm
        run: cargo install --locked cargo-generate-rpm

      # cargo generate-rpm does NOT auto-build — build the release binary first.
      # NO -lhidapi-hidraw (Fedora unified hidapi, spec §2).
      - name: Build release binary
        run: cargo build --release

      - name: Build .rpm
        run: cargo generate-rpm

      # cargo-generate-rpm emits target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm
      # (the "-1" is the RPM Release from release="1"). Release ASSET name drops
      # the -1: qmkonnect-<ver>-linux-x86_64.rpm.
      - name: Rename to release asset name
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          set -eux
          mv "target/generate-rpm/qmkonnect-${VERSION}-1.x86_64.rpm" \
             "qmkonnect-${VERSION}-linux-x86_64.rpm"

      # Append the .rpm to the release created by `publish` (additive; --clobber
      # idempotent on re-runs). gh came from the dnf step (above).
      - name: Upload .rpm to the GitHub Release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          gh release upload "${GITHUB_REF_NAME}" \
            "qmkonnect-${VERSION}-linux-x86_64.rpm" --clobber
  # ── End Task 1 Block ───────────────────────────────────────────────────────

Task 2: VERIFY the workflow YAML is well-formed (no other file touched)
  - RUN: python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml')); print('YAML OK')"
        (yaml is usually available; if not, use: node -e "require('js-yaml').load(require('fs').readFileSync('.github/workflows/release.yml','utf8')); console.log('YAML OK')"
        — but a plain structural grep is also a fine sanity check.)
  - RUN: grep -nE '^\s*rpm:|^  rpm:' .github/workflows/release.yml   # assert exactly ONE rpm: job key (column 0)
  - RUN: grep -nA2 '^  needs: \[publish\]' .github/workflows/release.yml | grep -q 'rpm' && echo "rpm needs publish (verify contextually)" || true
  - RUN: awk '/^publish:/{f=1} f&&/needs:/{print; f=0}' .github/workflows/release.yml | grep -qv 'rpm' && echo "PASS: rpm NOT in publish.needs" || echo "CHECK: rpm should not be in publish.needs"
  - EXPECT: YAML parses; exactly one top-level `rpm:` job; `rpm` is NOT inside the publish.needs list.
  - DEPENDENCIES: Task 1.

Task 3: REGRESSION GUARD (no source touched, but confirm the repo still builds/tests)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
        (single-threaded — AGENTS.md: the debouncer uses shared global state.)
        FALLBACK only if this dev box lacks the Linux GTK/X11 build deps:
          cargo check --bin qmkonnect   # must compile cleanly; record why the full test couldn't run.
  - EXPECT: green (this task changed ONLY release.yml — no .rs, no Cargo.toml —
        so the result must be identical to the prior green baseline).
  - DEPENDENCIES: Task 1 (none for the test itself, but logically last).
```

### Implementation Patterns & Key Details

```yaml
# The rpm job is a fusion of TWO existing proven jobs in the SAME file:
#   - the `deb` job: post-publish structure (needs:[publish], if:push, perms,
#     version step, rename step, gh release upload --clobber)
#   - the `arch` job: the container skeleton (runs-on:ubuntu-latest +
#     container:<img>:latest, deps-first step because the image has no git)
# Fused with THREE Fedora-specific deviations (the gotchas):
#   - rust via dtolnay/rust-toolchain (NOT dnf rust — MSRV)
#   - libappindicator-gtk3-devel (NOT libayatana — not in Fedora main)
#   - gh via dnf (NOT preinstalled — only ubuntu runners have it)
# And ONE invariant preserved: NO -lhidapi-hidraw (Fedora unified hidapi, §2).

# Step order (load-bearing): deps → checkout → toolchain → ver →
#   cargo install cargo-generate-rpm → cargo build --release →
#   cargo generate-rpm → rename → gh release upload.
# cargo generate-rpm does NOT build (run cargo build first); the rename source
# carries "-1" (RPM Release) which the asset name drops.
```

### Integration Points

```yaml
GITHUB WORKFLOWS:
  - file: .github/workflows/release.yml
    change: INSERT one new top-level `rpm:` job after the `deb:` job.
  - preserve: all other jobs; the on:/env:/permissions:/jobs: keys; publish.needs
    (do NOT add rpm to it — rpm is post-publish, off the critical path).

PRECONDITIONS (consumed, NOT authored here — all produced by the parallel
sibling P1.M7.T2.S1; verify presence with grep/ls before relying on them):
  - Cargo.toml → [package.metadata.generate-rpm] (+ [package.metadata.generate-rpm.requires])
  - packaging/rpm/postin  (RPM %post scriptlet)
  - packaging/rpm/postun  (RPM %postun scriptlet, $1=0 erase-guarded)
  - packaging/rpm/README.md  (build+install doc)
  - packaging/linux/{udev/69-qmkonnect-rawhid.rules, systemd/qmkonnect.service.template,
    xdg/qmkonnect.desktop}, README.md, LICENSE  (the asset sources)

NO CHANGES TO:
  - any .rs, Cargo.toml, packaging/rpm/* (P1.M7.T2.S1), release.toml,
    .cargo/config.toml, spec/PACKAGING.md (read-only), or any other packaging/*
    channel (.deb / arch / aur / nix / homebrew / scoop / winget / asdf).
```

## Validation Loop

### Level 1: YAML & Structural (Immediate Feedback)

```bash
# Parse the workflow YAML (python yaml is usually present; js-yaml fallback below).
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml')); print('YAML OK')" \
  || node -e "require('js-yaml').load(require('fs').readFileSync('.github/workflows/release.yml','utf8')); console.log('YAML OK (js-yaml)')"

# Exactly ONE top-level `rpm:` job (column 0).
grep -nE '^rpm:' .github/workflows/release.yml   # expect exactly one match

# rpm is a sibling of deb (both after publish, both native Linux packages).
awk '/^deb:/{print NR": deb"} /^rpm:/{print NR": rpm"} /^aur:/{print NR": aur"}' \
  .github/workflows/release.yml   # expect deb < rpm < aur in line order

# rpm is post-publish + tag-only + container (mirror deb/arch).
grep -nA4 '^rpm:' .github/workflows/release.yml \
  | grep -E 'needs: \[publish\]|github.event_name == .push.|container: fedora:latest|permissions:|contents: write'

# rpm is NOT in publish.needs (off the critical path).
sed -n '/^publish:/,/^[a-z]/p' .github/workflows/release.yml | grep 'needs:' \
  | grep -qv 'rpm' && echo "PASS: rpm not in publish.needs" || echo "CHECK publish.needs"

# No -lhidapi-hidraw anywhere in the rpm job (or the file).
grep -n 'hidapi-hidraw' .github/workflows/release.yml && echo "FAIL: hidraw flag present" \
  || echo "PASS: no -lhidapi-hidraw in release.yml"

# The full -devel set is present (not just the spec 3-pack).
sed -n '/^rpm:/,/^[a-z]/p' .github/workflows/release.yml \
  | grep -E 'gtk3-devel|libappindicator-gtk3-devel|libX11-devel|libxcb-devel|hidapi-devel|systemd-devel|libxdo-devel|gcc|pkgconf-pkg-config'

# Rust from the action, NOT dnf.
sed -n '/^rpm:/,/^[a-z]/p' .github/workflows/release.yml | grep 'dtolnay/rust-toolchain'
sed -n '/^rpm:/,/^[a-z]/p' .github/workflows/release.yml | grep -q 'dnf install.* rust ' \
  && echo "FAIL: dnf rust present (MSRV risk)" || echo "PASS: no dnf rust"

# Rename + upload steps present with the right names.
sed -n '/^rpm:/,/^[a-z]/p' .github/workflows/release.yml \
  | grep -E 'qmkonnect-\$\{VERSION\}-1\.x86_64\.rpm|qmkonnect-\$\{VERSION\}-linux-x86_64\.rpm|gh release upload .* --clobber'

# Expected: all greps match; YAML parses; rpm between deb and aur; rpm not in publish.needs.
```

### Level 2: Local `.rpm` smoke (optional — proves the recipe end-to-end on Fedora)

```bash
# NOTE: this dev box is ARCH (split hidapi), NOT Fedora. For a LOCAL structural
# smoke only (proves cargo generate-rpm reads the P1.M7.T2.S1 metadata + emits a
# structurally-valid .rpm), build with the Arch-only hidraw workaround for the
# BINARY step, then package:
cargo install --locked cargo-generate-rpm 2>/dev/null || cargo install cargo-generate-rpm
RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release   # Arch-host workaround ONLY
cargo generate-rpm
RPM=$(ls target/generate-rpm/qmkonnect-*-1.x86_64.rpm 2>/dev/null | head -1)
test -n "$RPM" && echo "PASS local smoke: $RPM produced" || echo "(structural smoke skipped/failed — the AUTHORITATIVE build is Fedora CI, not this Arch host)"
# Document: the Arch-host flag is a LOCAL validation workaround; the CI job sets
# NO flag (Fedora unified hidapi). Do NOT bake the flag into release.yml.
```

### Level 3: CI Validation (the real gate — runs only on a tag push)

```bash
# This is a CI job; the authoritative validation is a real `v*` tag push.
# Dry-run path: push a throwaway tag OR trigger workflow_dispatch (rpm is skipped
# on workflow_dispatch — it is if: github.event_name == 'push' — so to EXERCISE
# rpm you must push a real tag). Steps to validate after implementation:

# 1. (Local, before tagging) Re-run Level 1 — all structural greps must pass.
# 2. Tag + push:
git tag v0.2.8-rc.rpm-smoke   # throwaway pre-release tag (contains '-' → prerelease)
git push origin v0.2.8-rc.rpm-smoke
# 3. Watch the Actions UI: the `Linux (.rpm via cargo-generate-rpm)` job must
#    go green AFTER `publish`, producing qmkonnect-<ver>-linux-x86_64.rpm
#    attached to the Release. The other jobs must be unaffected.
# 4. Verify the asset:
gh release view v0.2.8-rc.rpm-smoke --json assets \
  --jq '.assets[].name' | grep 'linux-x86_64.rpm'
# 5. (Optional) download + inspect:
gh release download v0.2.8-rc.rpm-smoke -p '*.rpm' -D /tmp
rpm2cpio /tmp/qmkonnect-*-linux-x86_64.rpm | cpio -t 2>/dev/null \
  | grep -E 'usr/bin/qmkonnect$|usr/lib/udev/qmkonnect-hid-id$|etc/xdg/autostart/qmkonnect\.desktop$'
# Expected: the .rpm is attached; payload has the P1.M7.T2.S1 assets at the §4 paths.
# Clean up the throwaway tag/release after validating.
```

### Level 4: Regression (no source touched)

```bash
# AGENTS.md mandates single-threaded (shared debouncer state). This task touched
# ONLY release.yml, so the result must equal the prior green baseline.
cargo test --bin qmkonnect -- --test-threads=1
# FALLBACK (only if this host lacks Linux GTK/X11 build deps): cargo check --bin qmkonnect
# (must compile cleanly; note why the full test couldn't run).
# Expected: all tests pass (unchanged from baseline).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `release.yml` parses as YAML; exactly one top-level `rpm:` job;
      it sits between `deb` and `aur`; it is `needs:[publish]` +
      `if: github.event_name == 'push'` + `container: fedora:latest` +
      `permissions: contents: write`.
- [ ] Level 1: NO `-lhidapi-hidraw` anywhere; Rust from
      `dtolnay/rust-toolchain@stable` (NOT dnf rust); the FULL `-devel` set in the
      dnf step (incl. `libappindicator-gtk3-devel`, not libayatana).
- [ ] Level 1: `rpm` is NOT in `publish.needs`.
- [ ] Level 3: a throwaway `v*` tag push makes the rpm job go green after
      `publish` and attaches `qmkonnect-<ver>-linux-x86_64.rpm` to the Release.
- [ ] Level 4: `cargo test --bin qmkonnect -- --test-threads=1` green.

### Feature Validation
- [ ] The `rpm:` job is present and structurally mirrors the `deb` job (post-
      publish, tag-only, gh upload --clobber) + the `arch` job (container,
      deps-first step).
- [ ] dnf installs the full Fedora `-devel` set (gcc, gtk3-devel, glib2-devel,
      libappindicator-gtk3-devel, libX11-devel, libxcb-devel, libxdo-devel,
      hidapi-devel, systemd-devel, pkgconf-pkg-config) + git + jq + gh.
- [ ] Step order is build-then-generate-rpm; the rename drops the `-1` RPM
      Release; the upload uses `gh release upload --clobber`.
- [ ] The Mode-A comment block documents: Fedora container (no native runner),
      full-build-dep rationale, MSRV→toolchain rationale, appindicator-package
      rationale, no-hidraw invariant, post-publish intent, P1.M7.T2.S1 + spec
      §4.4/§9 cross-refs.

### Code Quality Validation
- [ ] YAML indentation is consistent with sibling jobs (job key col 0, fields
      col 2, step keys col 4).
- [ ] No existing job/key disturbed — pure insertion of one job.
- [ ] The `if: github.event_name == 'push'` guard means dry-runs skip the job
      (matches the deb job, matches the contract).
- [ ] `permissions: contents: write` is job-scoped (not hoisted to workflow level).

### Documentation & Deployment
- [ ] The Mode-A comment block is thorough enough that a future maintainer
      understands why Fedora is a container, why the dep set is large, why Rust
      comes from the action, and why there's no hidraw flag.
- [ ] Commit message notes: (a) consumes P1.M7.T2.S1's metadata + scripts;
      (b) the spec §9 3-pack is a summary → full -devel set; (c) Fedora =
      container (no native runner); (d) MSRV → dtolnay toolchain (not dnf rust);
      (e) libappindicator-gtk3-devel (not libayatana — not in Fedora main).
- [ ] `spec/PACKAGING.md` NOT edited (read-only source of truth).

---

## Anti-Patterns to Avoid

- ❌ Don't use `runs-on: fedora-latest` — no such runner exists. Use
  `runs-on: ubuntu-latest` + `container: fedora:latest`.
- ❌ Don't install Rust via `dnf install rust cargo` — Fedora rust (~1.79–1.85)
  is below the MSRV (1.88). Use `dtolnay/rust-toolchain@stable`.
- ❌ Don't limit the dnf step to the spec §9 3-pack (`hidapi-devel libxdo-devel
  pkg-config`) — the binary won't compile (missing gtk3/X11/appindicator/systemd
  headers). Install the full `-devel` set (the deb job proves this for the apt
  equivalent).
- ❌ Don't reference `libayatana-appindicator*` on Fedora — it's not in the main
  repos. Use `libappindicator-gtk3-devel`.
- ❌ Don't put the `actions/checkout` step before the dnf step — the fedora
  container has no git; checkout fails. Install deps (incl. git) FIRST.
- ❌ Don't add `-lhidapi-hidraw` to the build — Fedora's unified hidapi
  auto-selects hidraw; the flag is Arch-only (spec §2).
- ❌ Don't run `cargo generate-rpm` before `cargo build --release` — it does NOT
  auto-build and would package a stale/missing `target/release/*`.
- ❌ Don't forget the `-1` in the rename SOURCE filename (`qmkonnect-<ver>-1.x86_64.rpm`,
  the RPM Release from `release="1"`) — the release ASSET name drops it.
- ❌ Don't add `rpm` to `publish.needs` — it's a post-publish appender; keeping it
  off the critical path means a .rpm failure never blocks the core release.
- ❌ Don't omit `gh` from the dnf step — it's preinstalled only on ubuntu runners,
  not in `fedora:latest`; `gh release upload` would fail with "command not found".
- ❌ Don't edit `Cargo.toml`, `packaging/rpm/*`, or `spec/*` — those are
  P1.M7.T2.S1 / read-only. This task is release.yml ONLY.
- ❌ Don't use `softprops/action-gh-release@v2` as the PRIMARY upload path unless
  `dnf install gh` fails — `gh release upload --clobber` is the 1:1 mirror of the
  deb job (keep them consistent). (action-gh-release is the documented fallback.)

---

## Confidence Score: 9/10

**Why 9, not 10:** The job block is a copy-paste-ready fusion of two PROVEN jobs
already in this exact workflow file (the `deb` job for post-publish structure +
`gh release upload --clobber`; the `arch` job for the `container:` +
deps-before-checkout skeleton), with three Fedora-specific deviations that are
each verified and explained (rust via dtolnay/rust-toolchain because Fedora rust
< MSRV 1.88; `libappindicator-gtk3-devel` because libayatana isn't in Fedora
main; `gh` via dnf because the container lacks it). Every gotcha — no-native
runner, MSRV, appindicator package, no-git-in-image, no-hidraw-flag, build-before-
generate-rpm, the `-1` rename, gh-not-preinstalled, omit-rust-cache, off-the-
critical-path — is enumerated with reasoning. The -1 is for end-to-end validation
that can ONLY fully run on a real `v*` tag push against GitHub (the structural
greps + local smoke prove the recipe, but the green job is the real gate; the
first tag run may surface a Fedora-package-name surprise that the research
mitigated but couldn't 100% rule out without running on Fedora). One-pass
implementation success is very high: the deliverable is a single YAML job block,
fully specified, derived from two in-repo precedents.