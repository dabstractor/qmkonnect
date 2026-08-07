# Research Notes — P1.M7.T1.S2 (Add .deb CI job to release.yml)

## Sources
- `.github/workflows/release.yml` (full read + `scout` agent recon)
- `spec/PACKAGING.md` §4.3 (`.deb` via cargo-deb) and §9 (CI Release) — authoritative
- P1.M7.T1.S1 PRP (the parallel `[package.metadata.deb]` task — read as contract)
- `Cargo.toml` (version 0.2.8, no metadata.deb yet — expected)

---

## CRITICAL FINDING #1 — Contract's apt list is INCOMPLETE for a real build

**Contract / spec §9 / spec §4.3 all say:** `apt install libhidapi-dev libxdo-dev pkg-config`.

**Reality:** `cargo deb` runs a full `cargo build --release` under the hood. The
qmkonnect Linux binary links `tao`/`tray-icon`/`gtk-3`/`glib`/`ayatana-appindicator`/
`x11`/`xcb`/`libudev`, so the build FAILS (missing headers) unless ALL the dev
packages are present.

**Proof:** the existing `linux-binary` job (same ubuntu-22.04 runner) installs the
FULL set and it is the proven-working pattern:
```
build-essential pkg-config libgtk-3-dev libglib2.0-dev libayatana-appindicator3-dev \
libx11-dev libxcb1-dev libxdo-dev libhidapi-dev libudev-dev
```
=> The `deb` job MUST install this UNION (the full linux-binary set). The
contract's 3-package list is the runtime-relevant subset only.

## CRITICAL FINDING #2 — `deb` is the ONLY post-publish job writing to THIS release

All other post-publish jobs (`aur`, `homebrew-tap`, `scoop-bucket`, `winget`,
`asdf-plugin`) push to EXTERNAL repos via deploy keys / PATs and inherit
`contents: read`. The `deb` job APPENDS an asset to the GitHub Release created by
`publish` — so it is the unique post-publish job needing `permissions: contents: write`.

## CRITICAL FINDING #3 — Upload method: `gh release upload --clobber`

Two viable methods to append an asset to an already-created release:
- (a) `gh release upload <tag> <file> --clobber` — purpose-built, explicitly
  additive, `gh` preinstalled on ubuntu runners. **RECOMMENDED.**
- (b) `softprops/action-gh-release@v2` with `tag_name:` + single `files:` entry —
  IS additive (preserves existing assets) but re-touches the release object.
  Use only as an alternative.

`gh release upload` requires write access — satisfied by
`permissions: contents: write` + default `secrets.GITHUB_TOKEN` (passed as
`GH_TOKEN`). The tag is `${GITHUB_REF_NAME}` (e.g. `v0.2.8`) on a `v*` tag push.

## FINDING #4 — cargo-deb output path & build behavior
- Default output: `target/debian/qmkonnet_<version>_amd64.deb` (UNDERSCORES).
- cargo-deb builds the binary itself (`cargo build --release`); no separate build
  step required (but rust-cache still warms).
- `cargo install --locked cargo-deb` works on ubuntu-22.04; `--locked` for
  reproducible installs.

## FINDING #5 — scout's Model A vs the CONTRACT

The `scout` agent (analyzing the file in isolation) recommended Model A: make
`deb` a build job feeding `publish` (add to `publish.needs` + a `files:` glob).
**But the work-item CONTRACT is explicit:** `needs: [publish]` and
`if: github.event_name == 'push'` + "upload to the GitHub Release." The contract
wins. => Post-publish job that builds + uploads directly (NOT a build job feeding
publish, and the `publish` job is NOT modified).

Rationale for the contract's design: a `cargo install cargo-deb` + `cargo deb`
build is slow; keeping it off the publish critical path means a .deb build failure
doesn't block the core release (dmg/exe/tarball/pkg already published).

## FINDING #6 — Version detection
Rust-building jobs use the `cargo metadata | jq` variant (id: ver). Build-less
jobs use `${GITHUB_REF_NAME#v}`. The `deb` job BUILDS rust => use the
`cargo metadata | jq` variant.

## FINDING #7 — Job prologue (clone linux-binary)
Standard: `actions/checkout@v4` → `dtolnay/rust-toolchain@stable` →
`Swatinem/rust-cache@v2`, then apt install, then version step.

## FINDING #8 — Documentation deliverable = Mode-A comment block in release.yml
Contract point 5: "[Mode A] Comment block documenting the job in the release.yml."
=> NO separate doc file; the documentation is the YAML comment header on the job
(what it builds, why ubuntu-22.04, why no hidraw flag, why contents:write, the
glibc baseline, the post-publish rationale).

## Rename mapping
- Source (cargo-deb, underscores): `target/debian/qmkonnect_${VERSION}_amd64.deb`
- Target (release, dashes):       `qmkonnect-${VERSION}-linux-amd64.deb`
where `${VERSION}` = bare Cargo version (0.2.8).