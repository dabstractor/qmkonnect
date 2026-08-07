# Research Notes — P2.M7.T2.S1

**Work item:** Add GNOME extension zip CI job + Nix multi-arch check + linux-binary
tarball .desktop.
**Single file touched:** `.github/workflows/release.yml` (+ comment docs only).

---

## 1. Current state of `.github/workflows/release.yml`

Jobs in declared order (verified by reading the whole file):
`macos`, `windows`, `linux-binary`, `arch`, `publish`, `aur`, `homebrew-tap`,
`scoop-bucket`, `winget`, `asdf-plugin`.

- **There is NO `nix` job in release.yml.** The only Nix validation today lives
  in `.github/workflows/ci.yml` (`nix-check: nix flake check --no-build`). P2.M7.T2.S1
  is what adds the *release* nix job. (P1.M5.T2.S2 added the ci.yml one.)
- **There is NO `gnome-extension` job.** This task adds it.
- **The `.desktop` is ALREADY staged in `linux-binary`** — see finding §4. So
  requirement (c) is verification + docs only, NOT a code change.

### publish job (the wiring target) — exact lines
- L239: `name: Publish GitHub Release`
- L240: `needs: [macos, windows, linux-binary, arch]`
- `files:` glob block:
  ```
            files: |
              artifacts/macos-dmg/*
              artifacts/windows-exe/*
              artifacts/linux-binary/*
              artifacts/arch-pkg/*
  ```
  `softprops/action-gh-release@v2` + `actions/download-artifact@v4`
  (download path `./artifacts/<name>/`).

## 2. GNOME extension zip — format contract (PACKAGING.md §7 + §9)

- §9: "zip `packaging/gnome-shell-extension/` →
  `qmkonnect@mulletware.shell-extension.zip`, attach to the Release. (EGO upload
  is a manual maintainer step; CI just builds the zip.)"
- §7: "zip the directory as `qmkonnect@mulletware.shell-extension.zip`
  (the extensions.gnome.org upload format)."
- **uuid** = `qmkonnect@mulletware` (metadata.json) ⇒ zip name derived from it.
- Dir contents: `metadata.json`, `extension.js`, `stylesheet.css`,
  `dbus-interfaces.xml`, `README.md`. **No `schemas/`, no `po/`, no `prefs.js`.**
- **EGO/GNOME zip format: files at the ROOT of the archive** (no containing
  directory). This is what `gnome-extensions pack` produces and what
  `gnome-extensions install` + EGO expect. The #1 packaging mistake is nesting
  files under a top-level dir. ⇒ `cd packaging/gnome-shell-extension && zip`
  (contents), never `zip dir/`.
- `gnome-extensions pack` does NOT bundle arbitrary files like `README.md` /
  `dbus-interfaces.xml`. For EGO cleanliness (the release zip is also the manual
  EGO upload source), build the zip from an EXPLICIT file list
  (`extension.js metadata.json stylesheet.css`) rather than `zip .` .
- **version sync:** metadata.json has `"version": "0.2.8"` hardcoded; Cargo.toml
  is `0.2.8` (currently in sync). Every other release job injects the Cargo
  version; do the same with a TARGETED sed on the `"version"` field ONLY (must
  NOT touch the `"shell-version"` array):
  `sed -i -E 's/^(\s*"version"\s*:\s*")[^"]*(")/\1'"$VER"'\2/' metadata.json`

## 3. Nix multi-arch — the pivotal gotcha

- `flake.nix` declares `eachSystem [ "x86_64-linux" "aarch64-linux" ]` ⇒ BOTH
  systems are already real flake outputs.
- **`flake.nix` ships `cargoHash = pkgs.lib.fakeHash`** (a deliberate
  placeholder; qmk-notifier is a git dep so Cargo.lock lacks its vendor hash).
  A real `nix build .#qmkonnect` FAILS with a fixed-output hash mismatch until a
  human runs the one-time `nix build .#qmkonnect` → paste-`got:`-hash iteration.
  This is explicitly OUT OF SCOPE per ci.yml's comment block (lines 68-84).
- **Consequence:** the release.yml nix job CANNOT do a real `nix build` today
  without breaking every release (and if it gated `publish`, it would block ALL
  releases). The achievable, GREEN gate is `nix flake check --no-build`, which
  EVALUATES every flake output for BOTH systems (eachSystem expands to both).
  This is proven-green in ci.yml and is genuine multi-arch verification at the
  evaluation level.
- The task's `nix build .#qmkonnect --system x86_64-linux + aarch64` (or
  `nix flake check`) explicitly offers "`nix flake check`" as an acceptable
  alternative — we take it (`--no-build` variant) so the pipeline stays green.
- **nix job does NOT gate publish** (no artifact to publish; don't block the
  binary release on a verification signal / the fakeHash follow-up). It runs in
  parallel as a verification X (green today, red if a real eval regression lands).
- Future full-build path (DOCUMENT, don't implement): resolve cargoHash, enable
  qemu binfmt emulation (`cachix/install-nix-action` supports aarch64 emulation),
  then `nix build .#qmkonnect --system {x86_64,aarch64}-linux`.

## 4. Requirement (c) — `.desktop` in linux-binary tarball is ALREADY DONE

Commit `270df6c` ("Add XDG autostart entry and ship in Linux packages") already:
- L173: `mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"`
- L178: `cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"`
So the tarball already contains `qmkonnect-<ver>-linux-x86_64/xdg/qmkonnect.desktop`.
**No code change needed for (c).** The implementer only VERIFIES these lines
survive + that PACKAGING.md §4.6 (`sudo install -m644 .../qmkonnect.desktop
/etc/xdg/autostart/`) stays consistent. (No real `install.sh` script exists for
the tarball — §4.6 "install.sh" is an inline snippet; only `aur/publish.sh` and
the macOS scripts are real files.)

## 5. ci.yml nix-check (the existing proven pattern to mirror)
- `runs-on: ubuntu-latest`
- `cachix/install-nix-action@v31` with `extra_nix_config: access-tokens =
  github.com=${{ secrets.GITHUB_TOKEN }}` (additive; lifts GitHub API rate limit
  when nix re-resolves flake inputs — flake.lock is absent, so each run
  re-resolves nixpkgs + flake-utils).
- Flakes + nix-command are enabled by the action by default.
- Step: `nix flake check --no-build`.

## 6. Convention reminders for the implementer
- release.yml's strong convention: every job is preceded by a large `# ───`
  banner comment block documenting what it does + secrets + gotchas (Mode A docs).
  Match this exactly for the two new jobs.
- Version is read from `cargo metadata` everywhere; `GITHUB_REF_NAME#v` only in
  build-less publish jobs. The gnome-extension job BUILDS nothing but is an
  artifact producer → use the `cargo metadata` version pattern (like
  macos/linux-binary) OR the tag-strip pattern (like homebrew). Either is fine;
  tag-strip is simpler for a build-less job.
- `if-no-files-found: error` on every upload-artifact (existing convention).