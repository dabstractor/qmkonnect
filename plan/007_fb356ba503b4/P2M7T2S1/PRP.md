# PRP — P2.M7.T2.S1: GNOME extension zip CI job + Nix multi-arch check + linux-binary tarball .desktop

> **PRD context:** PACKAGING.md §9 (CI Release) + §7 (GNOME Shell Extension
> Artifact) + §4.6/§4.7 (Generic tarball / XDG `.desktop`). Selected PRD sections:
> F8/F15/F16/F17 feature rows + Linux packaging + cross-platform channels.
>
> **Single source file touched:** `.github/workflows/release.yml` (comment docs
> only, "Mode A"). No code, no scripts, no manifests outside that one file.

---

## Goal

**Feature Goal**: Extend the release CI pipeline with (a) a `gnome-extension`
job that packages the GNOME Shell extension into the extensions.gnome.org upload
zip and attaches it to the GitHub Release, (b) a `nix` job that verifies the
flake evaluates cleanly for **both** `x86_64-linux` and `aarch64-linux`, and
(c) confirm the XDG autostart `.desktop` is shipped in the generic Linux tarball.

**Deliverable**: Updated `.github/workflows/release.yml` containing a new
`gnome-extension` job (artifact producer, wired into `publish`), a new `nix`
job (verification-only, parallel, does **not** gate `publish`), and the
`.desktop` already-present in the `linux-binary` staging step (verification +
comment confirmation, no code change).

**Success Definition**:
- `release.yml` parses as valid YAML and `actions/download-artifact@v4` /
  `softprops/action-gh-release@v2` globs resolve.
- A `workflow_dispatch` dry-run produces a downloadable
  `qmkonnect@mulletware.shell-extension.zip` artifact whose entries are at the
  **archive root** (no containing directory) and whose `metadata.json` `version`
  matches the Cargo version.
- The `nix` job runs `nix flake check --no-build` and passes GREEN (no fakeHash
  failure) on every run.
- The `linux-binary` tarball still contains `xdg/qmkonnect.desktop`.
- The `publish` job (on a real tag push) attaches the extension zip to the
  GitHub Release; the `nix` job running/failing does not block `publish`.

---

## Why

- **GNOME extension distribution (F16):** GNOME/Mutter cannot report the active
  window to client processes, so a Shell extension (`qmkonnect@mulletware`,
  built in P2.M3.T1.S1) is the only reliable bridge. Users install it from a
  Release `.zip`; a CI job that builds that zip on every release makes the
  EGO-upload-source + direct-download asset reproducible (EGO upload itself stays
  a manual maintainer step per §7/§9).
- **Nix flake trust (F15):** the flake already declares
  `eachSystem [ "x86_64-linux" "aarch64-linux" ]`. A release-time verification
  that both systems evaluate catches regressions (e.g. a system-specific dep or
  `meta.platforms` typo) before a tagged release ships a broken flake.
- **Universal autostart (F17):** the `.desktop` must be in every Linux package —
  including the generic tarball — so login-autostart works on non-systemd distros
  (MX/Artix/Void/Gentoo). This was wired in the prior task (P2.M6.T1.S1); this
  task confirms it survives the CI refactor and documents it in the job banner.

---

## What

### User-visible behavior
- Each GitHub Release gains a `qmkonnect@mulletware.shell-extension.zip` asset
  (zip contents at archive root: `extension.js`, `metadata.json`,
  `stylesheet.css`; `metadata.json.version` = the release version).
- Each release pipeline run shows a green `nix` verification job evaluating both
  Linux architectures.
- The `qmkonnect-<ver>-linux-x86_64.tar.gz` asset continues to contain
  `xdg/qmkonnect.desktop` alongside the binaries, udev rule, and service template.

### Success Criteria
- [ ] New `gnome-extension` job builds the zip in the EGO format (root-level
      entries) and uploads it as artifact `gnome-extension`.
- [ ] `publish.needs` includes `gnome-extension`; `publish.files` globs
      `artifacts/gnome-extension/*`.
- [ ] New `nix` job runs `nix flake check --no-build`; job is **not** in
      `publish.needs`.
- [ ] `linux-binary` staging still `cp`s `qmkonnect.desktop` into `$STAGE/xdg/`.
- [ ] Every new/edited job carries the repo's `# ───` banner comment block
      (Mode A docs); the GNOME banner notes EGO upload is manual.

---

## All Needed Context

### Context Completeness Check
_If someone knew nothing about this codebase, would they have everything needed
to implement this successfully?_ **Yes** — exact YAML snippets, exact edit
points (with current line numbers), the pivotal `fakeHash` gotcha, and the EGO
zip-format contract are all below.

### Documentation & References

```yaml
# MUST READ — the spec source-of-truth for what each job must do.
- file: spec/PACKAGING.md
  why: "§9 CI Release — enumerates every release.yml job, including the TARGET
        spec lines: 'Nix job: nix build .# (x86_64 + aarch64) to verify the
        flake; no artifact to publish' and 'GNOME extension job (NEW): zip
        packaging/gnome-shell-extension/ -> qmkonnect@mulletware.shell-extension.zip,
        attach to the Release. (EGO upload is a manual maintainer step; CI just
        builds the zip.)'. §4.6/§4.7 define the tarball layout + the .desktop."
  section: "## 9. CI Release (line ~463); ## 7. (line ~400); ### 4.6 (line ~264); ### 4.7 (line ~279)"

- file: spec/PACKAGING.md
  why: "§7 spells out the zip BUILD contract verbatim: 'zip the directory as
        qmkonnect@mulletware.shell-extension.zip (the extensions.gnome.org
        upload format)' + the metadata.json shell-version bumping rule."
  pattern: "GNOME extension artifact contents + D-Bus contract"

- file: .github/workflows/release.yml
  why: "THE file to edit. Read the whole file first: the macos/windows/linux-binary/
        arch jobs show the exact Determine-version + upload-artifact + banner-
        comment conventions to mirror; the publish job (L239-252) shows the
        needs: + files: glob wiring; the homebrew-tap job shows the build-less
        GITHUB_REF_NAME#v version pattern usable for the build-less gnome job."

- file: .github/workflows/ci.yml
  why: "Lines 63-109: the PROVEN-GREEN nix job to mirror in release.yml —
        cachix/install-nix-action@v31 + access-tokens extra_nix_config + the
        load-bearing --no-build explanation. Copy this job's shape verbatim."
  pattern: "nix-check job (eval-only, fakeHash-safe)"

- file: flake.nix
  why: "Confirms eachSystem [ x86_64-linux aarch64-linux ] (so --no-build verifies
        BOTH arches) and the cargoHash = pkgs.lib.fakeHash blocker (a real
        nix build FAILS until a human pastes the real vendor hash — explicitly
        out of scope per ci.yml comments)."

- file: packaging/gnome-shell-extension/metadata.json
  why: "uuid=qmkonnect@mulletware (drives zip name); version field to sync from
        Cargo.toml; shell-version array (must NOT be touched by the version sed)."
  gotcha: "No schemas/, po/, or prefs.js in the dir, so a content zip is exactly
           equivalent to `gnome-extensions pack` output."

# EXTERNAL — the EGO zip-format convention (files at archive root).
- url: https://gjs.guide/extensions/development/creating.html#packaging-the-extension
  why: "`gnome-extensions pack` produces a zip with extension.js + metadata.json
        (+ stylesheet.css) at the ARCHIVE ROOT (no containing dir). `gnome-extensions
        install <uuid>.shell-extension.zip` and the EGO uploader both require this
        layout. Nesting files under a dir is the #1 packaging mistake."
  critical: "Build the zip from INSIDE packaging/gnome-shell-extension with an
             explicit file list, NOT `zip -r out.zip packaging/gnome-shell-extension`
             (which nests) and NOT `zip -r out.zip .` (which bundles README.md +
             dbus-interfaces.xml that gnome-extensions pack omits)."
- url: https://github.com/cachix/install-nix-action#inputs
  why: "install-nix-action enables flakes + nix-command by default; access-tokens
        extra_nix_config is additive and lifts the GitHub API rate limit when nix
        re-resolves flake inputs (flake.lock is absent)."
```

### Current Codebase tree (relevant slice)

```bash
.github/workflows/
  release.yml          # EDIT — add gnome-extension + nix jobs, wire publish
  ci.yml               # READ — proven nix-check job to mirror
packaging/
  gnome-shell-extension/        # INPUT — zipped by the new job
    metadata.json               #   uuid, version (sync), shell-version (do NOT touch)
    extension.js                #   required zip entry
    stylesheet.css              #   required zip entry
    dbus-interfaces.xml         #   NOT bundled (gnome-extensions pack omits)
    README.md                   #   NOT bundled (gnome-extensions pack omits)
  linux/xdg/
    qmkonnect.desktop           # INPUT — ALREADY staged by linux-binary (verify only)
spec/PACKAGING.md               # READ — §7/§9/§4.6/§4.7 spec
flake.nix                       # READ — eachSystem both arches + fakeHash blocker
Cargo.toml                      # READ — single source of truth for `version`
```

### Desired Codebase tree after this task

```bash
.github/workflows/release.yml   # +2 jobs (gnome-extension, nix) + 2 publish wiring edits
# (No new files. No new scripts. No new artifacts committed — *.shell-extension.zip
#  is gitignored per PACKAGING.md §11.)
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL — flake.nix cargoHash is a deliberate fakeHash placeholder (qmk-notifier
# is a git dep, so Cargo.lock lacks its vendor hash). A REAL `nix build .#qmkonnect`
# FAILS with a fixed-output hash mismatch. ci.yml already works around this with
# `nix flake check --no-build`. The release.yml nix job MUST use --no-build too,
# or it goes RED on every release (and if it gated publish, it would block ALL
# releases). This is explicitly a separate flake.nix follow-up, NOT in scope here.
# => nix job step = `nix flake check --no-build`. Do NOT use `nix build .#`.

# CRITICAL — GNOME extension zip must have files at the ARCHIVE ROOT (EGO format).
# Build it from INSIDE the dir with an explicit file list. Nesting under a
# containing dir breaks `gnome-extensions install` AND the EGO uploader.

# GOTCHA — version sed on metadata.json must touch ONLY the "version" field, never
# the "shell-version" array. Use an anchored regex on the `"version": "..."` line.

# CONVENTION — every release.yml job has a `# ───` banner comment block (Mode A
# docs) explaining the job + secrets + gotchas. Match it for the two new jobs.

# CONVENTION — nix produces NO publishable artifact ("consumed in-place"), so it
# is NOT added to publish.needs. gnome-extension IS an artifact producer, so it
# MUST be in publish.needs + publish.files.
```

---

## Implementation Blueprint

### Data models and structure
None — this task is pure CI YAML + comments. No Rust, no schemas, no migrations.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY the .desktop is already in the linux-binary tarball (NO CHANGE)
  - INSPECT release.yml lines ~173 + ~178:
      `mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"` and
      `cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"`
  - CONFIRM both lines exist (added by commit 270df6c / P2.M6.T1.S1).
  - RESULT: requirement (c) is already satisfied. If (and only if) the lines are
    missing, ADD them mirroring the existing udev/systemd staging pattern. Do not
    otherwise touch this job.
  - DOCS: ensure the linux-binary banner comment mentions the xdg/ .desktop
          (one line) so PACKAGING.md §4.6/§4.7 and the job stay in sync.

Task 2: ADD the `gnome-extension` job (artifact producer) to release.yml
  - INSERT a new job between the `arch` job and the `publish` job (keeps all
    artifact producers before publish). Use the banner-comment style of the
    macos/windows jobs. COPY-PASTE the YAML from "Task 2 snippet" below.
  - KEY DETAILS the snippet encodes:
      * build-less => version via the tag-strip pattern (`${GITHUB_REF_NAME#v}`),
        identical to homebrew-tap/scoop-bucket (simpler than cargo metadata here).
      * sed metadata.json "version" ONLY (anchored regex, never shell-version).
      * zip from INSIDE the dir with an explicit file list (EGO root format;
        omits README.md + dbus-interfaces.xml like gnome-extensions pack).
      * upload-artifact@v4, name: gnome-extension, if-no-files-found: error.
  - NAMING/PLACEMENT: job key `gnome-extension:`; runs-on: ubuntu-latest;
    NO needs (it is a leaf producer consumed only by publish).

Task 3: ADD the `nix` job (verification-only) to release.yml
  - INSERT right after the gnome-extension job (before publish). Banner comment
    MUST document: (a) verifies BOTH arches via eachSystem; (b) the fakeHash
    blocker => --no-build; (c) NO artifact => not in publish.needs; (d) the
    future full-build path (resolve cargoHash + binfmt emulation).
  - MIRROR ci.yml's nix-check job shape verbatim (cachix/install-nix-action@v31 +
    access-tokens extra_nix_config + `nix flake check --no-build`).
  - COPY-PASTE the YAML from "Task 3 snippet" below.
  - NAMING/PLACEMENT: job key `nix:`; runs-on: ubuntu-latest; NOT in publish.needs.

Task 4: WIRE gnome-extension into the publish job (2 edits)
  - EDIT publish.needs (release.yml ~L240):
      FROM: `needs: [macos, windows, linux-binary, arch]`
      TO:   `needs: [macos, windows, linux-binary, arch, gnome-extension]`
    (Do NOT add `nix` — it produces no artifact and must not gate the binary
     release on the fakeHash follow-up.)
  - EDIT publish.files glob (release.yml ~L248-252): add a line
      `            artifacts/gnome-extension/*`
    immediately after `            artifacts/arch-pkg/*`.
  - PRESERVE: existing globs, generate_release_notes, prerelease predicate.

Task 5: VALIDATE (see Validation Loop) — YAML lint + workflow_dispatch dry-run.
```

#### Task 2 snippet — `gnome-extension` job (copy-paste; place after `arch`, before `publish`)

```yaml
  # ─────────────────────────────────────────────────────────────────────────
  # GNOME Shell extension — zip packaging/gnome-shell-extension/ into the
  # extensions.gnome.org upload format and attach to the Release.
  #
  # Build-LESS: the extension is plain JS/JSON/CSS, so this job stages NO Rust
  # toolchain. The zip is the SAME artifact users `gnome-extensions install`
  # AND the source a maintainer manually uploads to extensions.gnome.org (EGO).
  # EGO upload itself is a MANUAL maintainer step (per PACKAGING.md §7/§9) — CI
  # only BUILDS the zip and attaches it here.
  #
  # EGO ZIP FORMAT (load-bearing): GNOME expects extension.js + metadata.json
  # (+ stylesheet.css) at the ARCHIVE ROOT — NO containing directory. Build the
  # zip from INSIDE the dir with an EXPLICIT file list so it (a) lands at root
  # and (b) omits README.md + dbus-interfaces.xml, exactly matching
  # `gnome-extensions pack` output. `gnome-extensions install` + the EGO uploader
  # both reject zips that nest files under a dir.
  #
  # VERSION SYNC: metadata.json ships a hardcoded `"version"`; patch it from the
  # release tag (strip the leading 'v', like the homebrew/scoop jobs). The regex
  # is anchored to the `"version": "..."` line ONLY — it MUST NOT touch the
  # `"shell-version"` array (EGO gates compatibility by that array).
  # ─────────────────────────────────────────────────────────────────────────
  gnome-extension:
    name: GNOME Shell extension (.zip)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # Build-less: GITHUB_REF_NAME is the tag (v0.2.8); strip the 'v' for the
      # bare version metadata.json expects (it rejects a leading 'v').
      - name: Determine version
        id: ver
        run: echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"

      # Patch metadata.json's "version" field ONLY. Anchored regex on the
      # `"version": "<digits.dots>"` line; the "shell-version" array is untouched.
      - name: Stamp version into metadata.json
        working-directory: packaging/gnome-shell-extension
        env:
          VERSION: ${{ steps.ver.outputs.version }}
        run: |
          sed -i -E 's/^(\s*"version"\s*:\s*")[^"]*(")/\1'"$VERSION"'\2/' metadata.json
          grep -E '"version"\s*:\s*"'"$VERSION"'"' metadata.json  # assert it landed

      # EGO format: entries at the archive ROOT (no containing dir). Explicit
      # file list = exactly what `gnome-extensions pack` bundles (no README, no
      # introspection XML). `zip` is preinstalled on ubuntu-latest.
      - name: Build extension zip (EGO upload format)
        working-directory: packaging/gnome-shell-extension
        run: |
          zip -r "$GITHUB_WORKSPACE/qmkonnect@mulletware.shell-extension.zip" \
            extension.js metadata.json stylesheet.css
          # Assert files are at the archive ROOT (no containing directory).
          unzip -l "$GITHUB_WORKSPACE/qmkonnect@mulletware.shell-extension.zip"

      - uses: actions/upload-artifact@v4
        with:
          name: gnome-extension
          path: qmkonnect@mulletware.shell-extension.zip
          if-no-files-found: error
```

#### Task 3 snippet — `nix` job (copy-paste; place after `gnome-extension`, before `publish`)

```yaml
  # ─────────────────────────────────────────────────────────────────────────
  # Nix flake — verify the flake evaluates cleanly for BOTH x86_64-linux AND
  # aarch64-linux (PRD F15; PACKAGING.md §9: "verify the flake; no artifact to
  # publish"). Verification-only; the flake is consumed in-place by users
  # (`nix run github:dabstractor/qmkonnect`), so this job uploads NOTHING and is
  # NOT in publish.needs (a flake regression must not block the binary release).
  #
  # WHY --no-build (load-bearing): flake.nix ships with
  #   cargoHash = pkgs.lib.fakeHash   (a deliberate placeholder)
  # because qmk-notifier is a git dependency and Cargo.lock does not carry its
  # vendor hash. A REAL `nix build .#qmkonnect` FAILS with a fixed-output hash
  # mismatch until a human runs the one-time `nix build .#qmkonnect` → paste the
  # "got: sha256-…" into flake.nix iteration. That flake.nix follow-up is OUT OF
  # SCOPE for this task (it is tracked separately; ci.yml's nix-check documents
  # the same blocker).
  #
  # `nix flake check --no-build` EVALUATES every flake output for EVERY declared
  # system. flake.nix's `eachSystem [ "x86_64-linux" "aarch64-linux" ]` means
  # --no-build verifies packages/devShells for BOTH architectures without
  # instantiating a build — so this is genuine MULTI-ARCH verification that stays
  # GREEN today. (The sibling ci.yml job uses the identical command.)
  #
  # FUTURE full-build path (DOCUMENT ONLY — do not enable until cargoHash is
  # resolved): enable aarch64 qemu binfmt emulation
  # (https://github.com/cachix/install-nix-action supports it) and run
  #   nix build .#qmkonnect --system x86_64-linux --no-link
  #   nix build .#qmkonnect --system aarch64-linux --no-link
  # ─────────────────────────────────────────────────────────────────────────
  nix:
    name: Nix flake check (x86_64-linux + aarch64-linux, eval)
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      # install-nix-action enables flakes + nix-command by default. access-tokens
      # (additive, not a replacement) prevents GitHub API rate limits when nix
      # re-resolves the flake inputs (flake.lock is absent, so each run re-
      # resolves nixpkgs + flake-utils). Mirrors ci.yml's nix-check job verbatim.
      - name: Install Nix
        uses: cachix/install-nix-action@v31
        with:
          extra_nix_config: |
            access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}

      - name: nix flake check (--no-build; both arches via eachSystem)
        run: nix flake check --no-build
```

### Implementation Patterns & Key Details

```python
# (YAML task, no code.) Key non-obvious points the snippets already encode:

# 1. GNOME zip = EGO root format. Verify with `unzip -l` that entries are
#    `extension.js`, `metadata.json`, `stylesheet.css` — NOT
#    `packaging/gnome-shell-extension/extension.js`.

# 2. metadata.json version sed is field-anchored; never rewrites shell-version.

# 3. nix job is --no-build (fakeHash blocker) and NOT in publish.needs.

# 4. gnome-extension IS in publish.needs + publish.files glob.
```

### Integration Points

```yaml
PUBLISH WIRING (.github/workflows/release.yml):
  - needs:  "[macos, windows, linux-binary, arch]"
    + ", gnome-extension"   # add gnome-extension (artifact producer)
    # do NOT add nix (no artifact; must not gate binary release)
  - files: |
      ...existing globs...
      artifacts/gnome-extension/*   # ADD this line after artifacts/arch-pkg/*

LINUX-BINARY (verify-only — already done at ~L173/L178):
  - mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"
  - cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"

NO OTHER FILES CHANGE: no Cargo.toml, no flake.nix, no scripts, no manifests.
```

---

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# YAML lint the only changed file (python + PyYAML is on every runner; locally
# use whatever you have — actionlint is the authoritative GH Actions check).
python3 -c "import yaml,sys; yaml.safe_load(open('.github/workflows/release.yml')); print('YAML OK')"

# Authoritative: actionlint catches malformed job/step/needs/expression syntax.
# (Preinstalled on ubuntu-latest via setup, or npx; if unavailable, rely on the
#  workflow_dispatch dry-run in Level 3 which fails the run on syntax errors.)
docker run --rm -v "$PWD":/repo -w /repo rhysd/actionlint:latest -color \
  .github/workflows/release.yml   || npx -y actionlint .github/workflows/release.yml

# Sanity: the two new job keys + the publish wiring are present and spelled right.
grep -nE '^  (gnome-extension|nix):'                 .github/workflows/release.yml
grep -n  'needs: \[macos, windows, linux-binary, arch, gnome-extension\]' \
                                                   .github/workflows/release.yml
grep -n  'artifacts/gnome-extension/\*'              .github/workflows/release.yml
grep -n  'qmkonnect@mulletware.shell-extension.zip'  .github/workflows/release.yml
grep -n  'nix flake check --no-build'                .github/workflows/release.yml
# Expected: each grep prints exactly the line(s) above; the .desktop lines at ~L173/L178 still present:
grep -n  'qmkonnect.desktop'                         .github/workflows/release.yml
```

### Level 2: Local Component Validation (no unit tests for YAML)

```bash
# Locally reproduce the GNOME zip step to PROVE the EGO format + version sync,
# independent of CI. (zip + unzip are on Linux/macOS; `@` in the name is fine.)
cd packaging/gnome-shell-extension
VER=9.9.9  # stand-in release version
sed -i -E 's/^(\s*"version"\s*:\s*")[^"]*(")/\1'"$VER"'\2/' metadata.json
grep -E "\"version\"\s*:\s*\"$VER\"" metadata.json    # assert
zip -r /tmp/qmkonnect@mulletware.shell-extension.zip extension.js metadata.json stylesheet.css
echo "--- zip listing (entries MUST be at root, no containing dir) ---"
unzip -l /tmp/qmkonnect@mulletware.shell-extension.zip
# Expected: extension.js, metadata.json, stylesheet.css listed with NO path prefix.
# Restore metadata.json so the local edit doesn't leak into the commit:
git checkout -- metadata.json

# Locally reproduce the nix eval gate IF nix is installed locally:
nix flake check --no-build   # from repo root; expect GREEN (eval-only, fakeHash-safe)
```

### Level 3: Integration Testing (System Validation — GitHub Actions)

```bash
# 1. Push the branch and trigger a workflow_dispatch DRY RUN (builds WITHOUT
#    publishing — the workflow's documented dry-run mode). On the Actions tab:
#    Release -> Run workflow -> (any branch with the change).
gh workflow run release.yml --ref <your-branch>    # then watch the run
# OR via the web UI: Actions -> Release -> Run workflow.

# 2. Download the gnome-extension artifact from the run and verify:
gh run download <run-id> -n gnome-extension -D /tmp/gnome-check
unzip -l /tmp/gnome-check/qmkonnect@mulletware.shell-extension.zip
# Expected: extension.js + metadata.json + stylesheet.css at the archive root.
cat /tmp/gnome-check/qmkonnect@mulletware.shell-extension.zip >/dev/null  # it's valid
unzip -p /tmp/gnome-check/qmkonnect@mulletware.shell-extension.zip metadata.json \
  | grep '"version"'   # Expected: matches the release version, NOT the stale 0.2.8

# 3. Confirm the nix job is GREEN and did NOT block publish's prereqs:
gh run view <run-id> --json jobs --jq '.jobs[] | {name, conclusion} | select(.name|test("nix|GNOME|Publish|Linux (binary"))'

# 4. Confirm the linux-binary tarball STILL contains the .desktop (download +
#    inspect — the xdg/ subdir must be present alongside udev/ and systemd/):
gh run download <run-id> -n linux-binary -D /tmp/linux-check
tar tzf /tmp/linux-check/qmkonnect-*-linux-x86_64.tar.gz | grep 'xdg/qmkonnect.desktop'

# Expected: the tarball lists .../xdg/qmkonnect.desktop.
```

### Level 4: Creative & Domain-Specific Validation

```bash
# Real-release parity (only on an actual v* tag push — NOT for the dry run):
# - publish must attach qmkonnect@mulletware.shell-extension.zip to the GitHub
#   Release (visible under the release's Assets).
# - The attached zip, fed to a real GNOME host, installs + enables cleanly:
#     gnome-extensions install qmkonnect@mulletware.shell-extension.zip
#     gnome-extensions enable qmkonnect@mulletware
#   (manual smoke test on a GNOME VM — out of CI's automated scope, documented in
#    packaging/gnome-shell-extension/README.md).
# - EGO manual upload: a maintainer uploads the SAME zip to extensions.gnome.org
#   (the `shell-version` array gates which GNOME lines it's offered for).
```

---

## Final Validation Checklist

### Technical Validation
- [ ] `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"` → OK
- [ ] `actionlint` clean (or a green workflow_dispatch run, which fails on syntax errors)
- [ ] grep checks in Level 1 all print the expected lines
- [ ] `nix flake check --no-build` green (local if nix present; CI otherwise)

### Feature Validation
- [ ] `gnome-extension` job present; zip has root-level entries (Level 2/3)
- [ ] metadata.json `version` matches the release version in the built zip
- [ ] `publish.needs` includes `gnome-extension`; `publish.files` globs `artifacts/gnome-extension/*`
- [ ] `nix` job present, NOT in `publish.needs`, runs `--no-build`
- [ ] `linux-binary` tarball still contains `xdg/qmkonnect.desktop`
- [ ] workflow_dispatch dry-run: all jobs green; gnome-extension + linux-binary artifacts downloadable
- [ ] On a real tag push: `qmkonnect@mulletware.shell-extension.zip` attached to the Release

### Code Quality / Docs
- [ ] Both new jobs carry the repo's `# ───` banner comment block (Mode A)
- [ ] GNOME banner explicitly notes EGO upload is a manual maintainer step
- [ ] Nix banner documents the fakeHash blocker + that --no-build verifies both arches + the future full-build path
- [ ] No new files, no Cargo/flake/manifest changes (single-file task)

---

## Anti-Patterns to Avoid

- ❌ Don't use a real `nix build .#qmkonnect` — `fakeHash` makes it RED every run
     (and gating publish on it would block all releases). Use `nix flake check
     --no-build`.
- ❌ Don't add `nix` to `publish.needs` — it has no artifact and must not gate the
     binary release on the unresolved fakeHash follow-up.
- ❌ Don't build the GNOME zip as `zip -r out.zip packaging/gnome-shell-extension`
     (nests files under a dir → EGO/install rejection) or `zip -r out.zip .`
     (bundles README.md + dbus-interfaces.xml). Use an explicit root-level file list.
- ❌ Don't let the metadata.json version sed touch the `"shell-version"` array.
- ❌ Don't re-add the `.desktop` staging to `linux-binary` — it's already there
     (commit 270df6c). Verify, don't duplicate.
- ❌ Don't create helper scripts or new files — this is a single-file YAML edit.

---

## Confidence Score

**9/10** — One-pass success is highly likely: the only changed file is a
well-understood workflow; both new jobs are given as complete, copy-paste YAML
mirroring proven-existing jobs (ci.yml's nix-check; macos/homebrew banner +
version patterns); the two pivotal gotchas (fakeHash → `--no-build`; EGO root
zip format) are encoded directly in the snippets; the `.desktop` requirement is
already satisfied (verification-only); and the publish wiring is two exact text
edits. The one residual risk (−1) is a first-run CI environment quirk in the
workflow_dispatch dry-run (e.g. `zip`/`unzip` PATH on a self-hosted runner), all
of which are GREEN on GitHub-hosted `ubuntu-latest` and caught by Level 3.