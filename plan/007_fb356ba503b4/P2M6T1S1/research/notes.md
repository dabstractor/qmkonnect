# Research Notes — P2.M6.T1.S1: XDG autostart `.desktop` + ship in all Linux packages

## 0. Scope of THIS task (verified against plan_status + item contract)

This task is the **owner + prerequisite** of the F17 XDG autostart `.desktop`.
It is consumed by 4 sibling tasks that are each still Planned/Ready (see §5).

### THIS task owns (the diff this task produces):
1. **CREATE** `packaging/linux/xdg/qmkonnect.desktop` — the file itself (PRIMARY).
2. **MODIFY** `.github/workflows/release.yml` — stage `xdg/qmkonnect.desktop` into
   the CI `linux-binary` tarball (enables AUR `-bin` to consume it).
3. **MODIFY** `packaging/linux/arch/PKGBUILD` — `package()` installs the `.desktop`
   to `$pkgdir/etc/xdg/autostart/qmkonnect.desktop` (644).
4. **MODIFY** `docs/installation.md` — Mode-A subsection: the autostart story
   (systemd user service vs XDG `.desktop`).

### NOT this task — owned by sibling Planned/Ready tasks (DO NOT EDIT):
| File / change | Owning task | Status |
|---|---|---|
| `packaging/linux/aur/PKGBUILD` (add `.desktop` extract+install) | **P1.M1.T1.S3** | Planned |
| `packaging/linux/aur/.SRCINFO` | **P1.M1.T1.S3** | Planned |
| `packaging/linux/aur/README.md` "same four paths" → five | **P1.M1.T1.S3** | Planned |
| `flake.nix` `postInstall` (install `.desktop`) | **P1.M1.T2.S3** | Planned |
| `Cargo.toml` `[package.metadata.deb]` assets (refs the `.desktop`) | **P1.M7.T1.S1** | Ready |
| `Cargo.toml` `[package.metadata.generate-rpm]` assets | **P1.M7.T2.S1** | Planned |
| `packaging/debian/*` + `packaging/rpm/*` | **P1.M7.T1.S1 / T2.S1** | Ready/Planned |
| Broad docs overhaul ("Hyprland Only" → cross-DE, troubleshooting) | **P2.M7.T1.S1** | Planned |
| `docs/llms_full.txt` regeneration | **P2.M7.T2.S2** | Planned |

This task creates the `.desktop` that all four consumers reference. The CI-tarball
change is what makes the file reachable for the AUR `-bin` PKGBUILD (P1.M1.T1.S3).

## 1. The authoritative `.desktop` contents (spec/PACKAGING.md §4.7 — VERBATIM)

```
[Desktop Entry]
Type=Application
Name=QMKonnect
Comment=Send the foreground window to your QMK keyboard
Exec=qmkonnect
Icon=input-keyboard
Terminal=false
X-GNOME-Autostart-enabled=true
Categories=Utility;
# Not shown in application menus (autostart-only):
NoDisplay=true
```
- Ship path: `/etc/xdg/autostart/qmkonnect.desktop` (mode 644), in EVERY package.
- `Exec=qmkonnect` relies on `/usr/bin` (or Nix store path) on `$PATH` at login;
  packages install to `/usr/bin`, so satisfied for .deb/.rpm/Arch/AUR.
- Disable = copy to `~/.config/autostart/qmkonnect.desktop` with `Hidden=true`
  (per-user overrides system-wide), or delete the system file.

## 2. The autostart STORY (spec/LINUX.md §6.3 — the Mode-A doc source)

- `/etc/xdg/autostart/qmkonnect.desktop` is the **universal fallback** alongside
  the systemd user service (§6.1).
- Every DE session manager honors `~/.config/autostart/` + `/etc/xdg/autostart/`
  → starts the daemon at **login on every desktop — systemd or not** (MX, Artix,
  Void, Gentoo).
- **Load-bearing** on non-systemd distros; **belt-and-suspenders** on systemd ones.
- **Trade-off vs the service**: the `.desktop` is login-only-start; it LOSES the
  systemd `BindsTo=dev-qmkonnect_device.device` plug/unplug lifecycle (start on
  plug, stop on unplug). On systemd distros the service stays primary; the
  `.desktop` is redundant-but-harmless (single-instance story owned by the
  tray/runner, NOT the launcher).
- F17 (PRD §4) is exactly this: "Universal Linux autostart: XDG autostart `.desktop`
  alongside the systemd user service, so login-autostart works on systemd AND
  non-systemd distros."

## 3. Current on-disk state (all confirmed ABSENT today — to be added)

```
$ ls packaging/linux/xdg/                    → (no such dir)
$ grep -n xdg desktop .github/workflows/release.yml  → (none)
$ grep -n xdg desktop autostart packaging/linux/arch/PKGBUILD  → (none)
```

## 4. The CI `linux-binary` tarball job (.github/workflows/release.yml L142-184)

Current staging (the ONLY place the tarball contents are assembled):
```yaml
  - name: Stage binary tarball
    run: |
      set -eux
      VER="${{ steps.ver.outputs.version }}"
      STAGE="qmkonnect-${VER}-linux-x86_64"
      mkdir -p "$STAGE/udev" "$STAGE/systemd"
      cp target/release/qmkonnect       "$STAGE/"
      cp target/release/qmkonnect-hid-id "$STAGE/"
      cp packaging/linux/udev/69-qmkonnect-rawhid.rules "$STAGE/udev/"
      cp packaging/linux/systemd/qmkonnect.service.template "$STAGE/systemd/"
      tar czf "${STAGE}.tar.gz" "$STAGE"
```
**Needed edit** (mirror the existing `udev`/`systemd` subdir pattern):
```yaml
      mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"
      ...
      cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"
```
This makes the tarball contain `qmkonnect-<ver>-linux-x86_64/xdg/qmkonnect.desktop`,
which P1.M1.T1.S3's AUR PKGBUILD will install as `${stage}/xdg/qmkonnect.desktop`.
- NOTE: the AUR PKGBUILD's `sha256sums` (L46) is pinned to the CURRENT tarball —
  P1.M1.T1.S3 must re-pin it after the tarball contents change. This task only
  changes the tarball; it does NOT touch the AUR PKGBUILD.

## 5. The Arch source PKGBUILD (packaging/linux/arch/PKGBUILD) — package()

Current `package()` installs 4 files via relative paths from `$srcdir/..`:
```bash
package() {
  cd "$srcdir/.."
  install -Dm755 "../../../target/release/qmkonnect" "$pkgdir/usr/bin/qmkonnect"
  install -Dm755 "../../../target/release/qmkonnect-hid-id" "$pkgdir/usr/lib/udev/qmkonnect-hid-id"
  install -Dm644 "../udev/69-qmkonnect-rawhid.rules" "$pkgdir/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules"
  install -Dm644 "../systemd/qmkonnect.service.template" "$pkgdir/usr/lib/systemd/user/qmkonnect.service.template"
}
```
**Needed edit** — add the 5th install (mirror the `../udev` / `../systemd` relative
pattern; the file is a STATIC shipped file, no instantiation, so NO hook change):
```bash
  # XDG autostart entry — universal login-autostart fallback (F17; LINUX.md §6.3).
  # Static file (no template instantiation); NoDisplay=true hides it from app menus.
  install -Dm644 "../xdg/qmkonnect.desktop" "$pkgdir/etc/xdg/autostart/qmkonnect.desktop"
```
- `packaging/linux/arch/qmkonnect.install` (pacman hooks) needs NO change — the
  `.desktop` is a static file; nothing to instantiate/reload. (post_install/
  post_upgrade/post_remove deal only with the systemd template + udev.)
- The `arch` CI job (release.yml L189-231) runs `makepkg` in `packaging/linux/arch`
  against the full checkout, so `../xdg/qmkonnect.desktop` resolves to
  `packaging/linux/xdg/qmkonnect.desktop` in the repo. Confirmed consistent.

## 6. Sibling consumers (NOT edited here — listed so the handoff is explicit)

- **AUR `-bin` (P1.M1.T1.S3)**: `packaging/linux/aur/PKGBUILD` `package()` will add
  `install -Dm644 "${stage}/xdg/qmkonnect.desktop" "${pkgdir}/etc/xdg/autostart/qmkonnect.desktop"`
  (mirrors its existing `${stage}/udev/...` + `${stage}/systemd/...` lines). It
  depends on §4's tarball change landing FIRST.
- **Nix (P1.M1.T2.S3)**: `flake.nix` `postInstall` adds the `.desktop` (NixOS uses
  systemd; the `.desktop` is belt-and-suspenders / for non-systemd). flake.nix
  currently ships udev helper + rule + systemd service (no `.desktop`).
- **.deb (P1.M7.T1.S1)**: `[package.metadata.deb] assets` already (per spec §4.3)
  includes `["packaging/linux/xdg/qmkonnect.desktop", "etc/xdg/autostart/", "644"]`.
  Cargo.toml has NO `[package.metadata.deb]` block yet — P1.M7.T1.S1 creates it.
- **.rpm (P1.M7.T2.S1)**: `[package.metadata.generate-rpm] assets` (per spec §4.4)
  includes the `.desktop` dest `/etc/xdg/autostart/qmkonnect.desktop`. Block absent
  today — P1.M7.T2.S1 creates it.

## 7. docs/installation.md (Mode-A docs deliverable)

Current `## Linux` section (L97+) has: "Linux (Hyprland Only)" header (now
outdated post-F16 — that overhaul is P2.M7.T1.S1, NOT this task), "Other Linux
Distributions" manual-install block (NO `.desktop` line), and package-manager
subsections (AUR/Nix/mise-asdf). The manual-install block (L143-152) currently
installs only binary + helper + udev rule — it does NOT match spec §4.6 (which
adds the `.desktop` install line).

**This task's docs change = MODE A**: a focused new subsection under `## Linux`
documenting the autostart story (service vs `.desktop`). Place it right after the
systemd-service manual step (L126-134) or as a dedicated `### Autostart at login`
subsection. Keep it concise: the two mechanisms, which wins, the trade-off, how to
disable. Optionally add the `install -m644 .../xdg/qmkonnect.desktop /etc/xdg/autostart/`
line to the existing manual-install snippet (spec §4.6 mandates it; it IS the
autostart story for manual installs). DO NOT rewrite the "Hyprland Only" header or
restructure the Linux section — that is P2.M7.T1.S1.

## 8. The .deb/.rpm install paths table — 7th file in the shared artifact set

spec/PACKAGING.md §4 table (the FHS path contract every package obeys):
```
| XDG autostart (new) | /etc/xdg/autostart/qmkonnect.desktop |
```
This is the **7th** row (binary, helper, udev rule, service template, instantiated
service, XDG autostart, docs). This task creates the source file; the table is
already in the spec (READ-ONLY).

## 9. Build-state note (the parallel P2.M5.T1.S1 / P2.M3.T2.S2 caveat)

`cargo build --release` (default features) is currently RED from the parallel
GNOME first-run-notify work reaching private `mod gnome;`
(`src/runners/linux.rs:194: error[E0603]`). That is NOT this task's concern and
NOT caused by it. This task's deliverables are ALL non-Rust (.desktop, YAML,
PKGBUILD, .md), so NO Rust build is part of the validation loop. If any gate
needs a build, use `cargo build --no-default-features` (the X11-only path, GREEN).
Validating the `.desktop` syntax uses `desktop-file-validate`, not cargo.