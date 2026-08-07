# Research Notes — P1.M7.T1.S1 (.deb via cargo-deb)

## Asset source-path existence (per PACKAGING.md §4.3 assets array)
```
OK      target/release/qmkonnect
OK      target/release/qmkonnect-hid-id
OK      packaging/linux/udev/69-qmkonnect-rawhid.rules
OK      packaging/linux/systemd/qmkonnect.service.template
ABSENT  packaging/linux/xdg/qmkonnect.desktop      ← P2.M6.T1.S1, hard prerequisite
OK      README.md
OK      LICENSE
```
→ The `.desktop` is a build blocker. Its content is 100% pinned by §4.7 (11-line
.desktop). PRP treats it as a precondition with a §4.7-content fallback so the
build is unblocked whether or not P2.M6.T1.S1 has landed (byte-identical either
way — idempotent).

## Host environment (this dev box)
- OS: **Arch Linux** (`/etc/os-release` ID=arch). `dpkg-deb` NOT installed.
- cargo 1.92.0 / rustc 1.92.0. `cargo-deb` NOT installed (needs `cargo install cargo-deb`).
- `target/release/{qmkonnect,qmkonnect-hid-id}` already built (Aug 6).
- Implication: local `cargo deb` may relink; on Arch the hidapi crate links the
  SPLIT `libhidapi-hidraw` lib → a plain build FAILS without
  `RUSTFLAGS="-C link-arg=-lhidapi-hidraw"` (that's exactly why the Arch PKGBUILD
  sets it). The .deb targets Debian's UNIFIED hidapi (no flag). So a full local
  build on Arch may fail at link; the authoritative build is ubuntu-22.04 CI
  (P1.M7.T1.S2). For local config/structure validation, use the hidraw-flag
  workaround OR rely on the existing up-to-date artifacts (cargo skips relink).

## cargo-deb config semantics (authoritative: crates.io + docs.rs cargo-deb README)
- `maintainer-scripts` = directory containing bare-named `preinst`/`postinst`/
  `prerm`/`postrm` files (NO `.sh` extension). cargo-deb installs them into the
  deb control archive verbatim.
- `assets` entries = `[source, target_dir, mode]`; source paths relative to
  Cargo.toml dir; `target/release/` allowed.
- `license-file = ["LICENSE", "0"]` → 2-element array: [path, lines-to-skip-at-top].
  `"0"` = skip none → whole LICENSE becomes `/usr/share/doc/qmkonnect/copyright`.
  (kornelski's own uses `"5"` to skip a preamble; ours has no preamble → `"0"`.)
  Coexists fine with package-level `license = "MIT"` (fills control `License:`).
- `extended-description-file` → file read as the long description (NOT the
  synopsis; synopsis = first line of Cargo.toml `description`).
- `cargo deb` runs `cargo build --release` internally (skips if up-to-date).
  Output: `target/debian/qmkonnect_<ver>_amd64.deb`.

## CRITICAL gotcha — cargo-deb systemd-unit auto-detection (#DEBHELPER#)
cargo-deb (PR #135, like dh_installsystemd) detects `.service`/`.socket`/`.timer`
unit files in asset target paths and AUTO-INJECTS debhelper fragments into
maintainer scripts, requiring a `#DEBHELPER#` token where they insert.
- Our package ships `qmkonnect.service.TEMPLATE` (not `.service`) → the filename
  does NOT end in `.service` → cargo-deb does NOT detect a unit → NO fragments
  generated → `#DEBHELPER#` token is NOT required and MUST NOT be added (a
  literal leftover `#DEBHELPER#` would be a bug). Our manual `systemctl --global
  enable` is the sole enablement mechanism. This mirrors the Arch package
  (ships `.service.template`, post_install instantiates it). SAFE BY DESIGN.

## CRITICAL gotcha — POSIX sh (dash), NOT bash
Debian maintainer scripts run under `/bin/sh` = dash. The Arch
`qmkonnect.install` uses the bashism `&>/dev/null` (redirect stdout+stderr) which
dash REJECTS. Must translate to POSIX `>/dev/null 2>&1`. Other constructs used
(heredoc `<<'EOF'`, `[ -d ]`, `for x in /home/*`, `id -u`, `su -c`, `basename`,
`2>/dev/null`) are all POSIX-safe.

## `input` group idempotency
Debian ships the `input` group by default. `addgroup --system input` errors if it
already exists → guard with `getent group input >/dev/null 2>&1 || addgroup --system input`.
`addgroup` is Debian-specific (adduser pkg); Arch uses `groupadd` (irrelevant —
these scripts only ever run on Debian/Ubuntu).

## Maintainer-script logic source of truth
`packaging/linux/arch/qmkonnect.install` (and the identical
`packaging/linux/aur/qmkonnect.install`) — `post_install`/`post_upgrade`/
`post_remove`. The Debian `postinst` mirrors `post_install`, `postrm` mirrors
`post_remove`, `prerm` is a no-op (contract §3c). `post_upgrade` has no Debian
equivalent in the contract (upgrade re-runs postinst with `$1=configure` after
unpack; the template re-instantiation + udev reload naturally re-runs there).

## depends line (§4.3, §4.8)
`libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd`
(libhidapi-hidraw0 = the hidraw backend the unified lib dlopens; libxdo3 = xdo
for window ops; zenity + libnotify-bin = Linux settings dialog + notifications;
systemd = the user service + --global). Build-deps (CI apt): `libhidapi-dev
libxdo-dev pkg-config`.

## Existing packaging README style (to mirror for packaging/debian/README.md)
`packaging/linux/aur/README.md` and `packaging/nix/README.md`: H1 title, "What
this is" section, "Install" with fenced commands, cross-links to
`spec/PACKAGING.md` and `docs/installation.md` via relative paths
(`../../../spec/PACKAGING.md` from packaging/debian/).

## Cargo.toml facts
- version = "0.2.8", description = "Cross-platform window activity notifier for
  QMK keyboards", license = "MIT", readme = "README.md".
- No `authors`/`maintainer`/`homepage`/`repository` at [package] level —
  [package.metadata.deb] will set maintainer/copyright explicitly.
- Two [[bin]]: qmkonnect (src/main.rs) + qmkonnect-hid-id (src/bin/hid_id.rs).
- [profile.release] last in file → append [package.metadata.deb] at EOF.