# Research — Container CI patterns for the `.rpm` job (runner, toolchain, gh, rust-cache)

## 1. There is NO native `fedora-latest` GitHub Actions runner

GitHub-hosted runners: `ubuntu-*`, `macos-*`, `windows-*`. No Fedora. The only
way to build on Fedora is:

```yaml
runs-on: ubuntu-latest
container: fedora:latest
```

This **exactly mirrors the proven `arch` job** in the SAME workflow
(`.github/workflows/release.yml`), which does:

```yaml
arch:
  runs-on: ubuntu-latest
  container: archlinux:latest
  steps:
    - name: Install build dependencies
      run: pacman -Sy --noconfirm --needed base-devel rust cargo git …   # BEFORE checkout
    - uses: actions/checkout@v4
```

So the `rpm` job is the **second** container-based job in this workflow — the
arch job is the load-bearing precedent that the `container:` + "install deps
before checkout (container ships no git)" pattern works here.

## 2. GOTCHA — do NOT install Rust via `dnf install rust` (MSRV violation)

The `arch` job installs rust via pacman (`pacman -Sy … rust cargo`) because
Arch rolling ships rust ≥1.88 (meets the MSRV in `Cargo.toml`).

**Fedora's packaged rust is too old:**
- `Cargo.toml`: `rust-version = "1.88"`  (MSRV floor)
- Fedora 41 `rust` ≈ 1.82; Fedora 40 ≈ 1.79; Fedora 42 ≈ 1.85 — **all < 1.88**.
- `dnf install rust cargo` would give a toolchain that FAILS the build
  (`rust-version` manifest check / missing 1.88 features).

**Use `dtolnay/rust-toolchain@stable`** (like the `deb` job — the native-package
sibling), which always installs the latest stable (≫1.88). It works inside a
`container:` (it's a JS action that downloads rustup-init; needs network + a
download tool, both present on GHA runners; `ca-certificates` in the dnf step
keeps HTTPS working).

```yaml
- uses: dtolnay/rust-toolchain@stable
```

⇒ The dnf step installs the **C/GTK** toolchain + git/jq/gh; the Rust toolchain
comes from the action, NOT from dnf. This is the one structural deviation from
the arch job (which gets rust from pacman) — and it is *required*.

## 3. rust-cache inside a container — OMIT (proven-safe by the arch precedent)

- The `deb` job uses `Swatinem/rust-cache@v2` but runs on a **native** ubuntu
  runner (no container).
- The `arch` job — the ONLY container job in this workflow — **deliberately
  omits** rust-cache.
- rust-cache inside a `container:` can hit UID/permission mismatches between
  the cache-restore action (runs as the GHA runner user) and the container
  user (root, since `dnf` needs root). The arch job avoids this by not caching.

**Decision: OMIT rust-cache** for the rpm job (mirror the arch container job —
proven one-pass-safe). A release tag is infrequent; building from scratch is
fine. (Documented optional optimization: add `Swatinem/rust-cache@v2` only after
verifying cache restore succeeds as root in the fedora container.)

## 4. Step ordering inside the container (mirror arch job)

```yaml
steps:
  - name: Install build dependencies          # dnf: git + full GTK/X11 stack + gh
    run: dnf install -y …                     # BEFORE checkout (no git in image)
  - uses: actions/checkout@v4
  - uses: dtolnay/rust-toolchain@stable       # (NOT dnf rust — MSRV)
  - name: Determine version
    id: ver
    run: |                                    # jq parsing (mirror deb job)
      v=$(cargo metadata --no-deps --format-version 1 | jq -r '…')
      echo "version=$v" >> "$GITHUB_OUTPUT"
  - name: Install cargo-generate-rpm
    run: cargo install --locked cargo-generate-rpm
  - name: Build release binary                # NO -lhidapi-hidraw (Fedora unified hidapi)
    run: cargo build --release
  - name: Build .rpm
    run: cargo generate-rpm                   # does NOT auto-build; build ran above
  - name: Rename to release asset name
    run: mv "target/generate-rpm/qmkonnect-${VERSION}-1.x86_64.rpm" \
              "qmkonnect-${VERSION}-linux-x86_64.rpm"
  - name: Upload .rpm to the GitHub Release   # gh from dnf; --clobber idempotent
    env: { GH_TOKEN: ${{ secrets.GITHUB_TOKEN }} }
    run: gh release upload "${GITHUB_REF_NAME}" "qmkonnect-${VERSION}-linux-x86_64.rpm" --clobber
```

## 5. `gh` (GitHub CLI) inside the fedora container

- The `deb` job uses `gh release upload … --clobber` (gh is preinstalled on
  ubuntu runners). The rpm job needs `gh` in the **container** — it is NOT
  preinstalled in `fedora:latest`.
- `dnf install gh` works — gh is in Fedora's main repo (per GitHub CLI install
  docs, `sudo dnf install gh`).
- **Fallback** (if `dnf install gh` ever fails): swap the upload step for the
  self-contained `softprops/action-gh-release@v2` JS action (no system gh; the
  `publish` job already uses @v2 in this repo so the version is vetted):
  ```yaml
  - uses: softprops/action-gh-release@v2
    with:
      tag_name: ${{ github.ref_name }}
      files: qmkonnect-${{ steps.ver.outputs.version }}-linux-x86_64.rpm
  ```
  But `gh release upload --clobber` is the cleanest 1:1 mirror of the deb job.

## 6. Why NO `-lhidapi-hidraw` (the build invariance, restated)

- Fedora/RHEL/Rocky/Alma ship a **UNIFIED** hidapi (≥0.14) in `libhidapi.so`
  that auto-selects the hidraw backend at runtime. (spec/PACKAGING.md §2.)
- Passing `-lhidapi-hidraw` would link Arch's split hidraw lib — WRONG on Fedora.
- ⇒ the job sets **no** RUSTFLAGS; plain `cargo build --release` is correct.
  (The Arch PKGBUILD — a different channel, built on archlinux — is the ONLY
  place the hidraw flag appears.)

## 7. Job dependency wiring

- `needs: [publish]` — runs AFTER `publish` creates the Release (so the tag +
  Release exist for `gh release upload`). Identical to the `deb` job.
- `if: github.event_name == 'push'` — tag-only; skipped on `workflow_dispatch`
  dry-runs (no Release to append to). Identical to the `deb` job.
- **NOT added to `publish.needs`** — like the `deb` job, the rpm job is a
  POST-publish appender, off the core release critical path. A .rpm build
  failure must NOT block the core release.

## 8. Placement

Insert the `rpm:` job **immediately after the `deb:` job** (they are siblings —
both native Linux packages, both post-publish appenders). The current order is
`… publish → deb → aur → …`; insert `rpm` between `deb` and `aur`.