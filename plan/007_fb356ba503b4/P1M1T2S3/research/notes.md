# P1.M1.T2.S3 — Research Notes

## Task
Add a 4th step to `flake.nix`'s `postInstall` that copies
`packaging/linux/xdg/qmkonnect.desktop` → `$out/etc/xdg/autostart/qmkonnect.desktop`.
Update `packaging/nix/README.md` to note the `.desktop` ships in the package.

## Codebase facts gathered

### Current `flake.nix` postInstall (3 steps, numbered `# 1.` `# 2.` `# 3.`)
- Step 1: udev helper `install -Dm755 $out/bin/qmkonnect-hid-id $out/lib/udev/qmkonnect-hid-id`
- Step 2: udev rule `install -Dm644 ${./packaging/linux/udev/69-qmkonnect-rawhid.rules} $out/lib/udev/rules.d/...` then `substituteInPlace` rewrites the helper path
- Step 3: systemd service `substitute ${./packaging/linux/systemd/qmkonnect.service.template} $out/lib/systemd/user/qmkonnect.service --replace "/usr/bin/qmkonnect" "$out/bin/qmkonnect"`

New step 4 must follow the SAME idiom: `install -Dm644 ${./packaging/linux/xdg/qmkonnect.desktop} $out/etc/xdg/autostart/qmkonnect.desktop`.
The `${./...}` Nix string interpolation COPIES the source file into the Nix store at eval time (same as steps 2 & 3) — this is correct.

### The `.desktop` file (`packaging/linux/xdg/qmkonnect.desktop`)
Created by P2.M6.T1.S1 (Complete). Contents: `Type=Application`, `Name=QMKonnect`,
`Comment=...`, `Exec=qmkonnect`, `Icon=input-keyboard`, `Terminal=false`,
`X-GNOME-Autostart-enabled=true`, `Categories=Utility;`, `NoDisplay=true`.

### Cross-package contract for the `.desktop` (CONSISTENCY — must match)
All Linux packages install the SAME static file to the SAME path. Verified:
- `packaging/linux/arch/PKGBUILD:51` → `$pkgdir/etc/xdg/autostart/qmkonnect.desktop`
- `packaging/linux/aur/PKGBUILD:68` → `${pkgdir}/etc/xdg/autostart/qmkonnect.desktop`
- `spec/PACKAGING.md` §4.7 table row → `/etc/xdg/autostart/qmkonnect.desktop`
- (Future .deb/.rpm at lines 172 & 223 — same path, mode 644)

So Nix step 4 → `$out/etc/xdg/autostart/qmkonnect.desktop` (mode 644). Consistent.

### CRITICAL: `Exec=qmkonnect` is a BARE name — DO NOT substitute
`spec/PACKAGING.md` §4.7 explicitly states:
> `Exec=qmkonnect` relies on `/usr/bin` (or the Nix store path) being on `PATH`
> in the session ... The Nix module and the `.desktop` are independent (NixOS
> uses systemd; the `.desktop` is for non-systemd or as a belt-and-suspenders).

Steps 2 & 3 substitute because they contain HARDCODED `/usr/...` absolute paths
that do not exist in Nix. The `.desktop` has NO absolute path — just `Exec=qmkonnect`
(PATH lookup). Substituting it to the store path would DEVIATE from the cross-package
contract and from §4.7. The binary is on PATH via:
- NixOS module: `environment.systemPackages = [ pkg ]` (already in nixosModule config)
- Non-NixOS: `nix profile install` → `~/.nix-profile/bin/qmkonnect` on PATH

### CRITICAL: `flake.lock` does NOT exist and must NOT be created here
`ls flake.lock` → No such file. This is BY DESIGN:
- `release.yml` nix job comment: "flake.lock is absent, so each run re-resolves
  nixpkgs + flake-utils"
- Inputs (`nixpkgs`, `flake-utils`) are NOT changing in this task
- Contract conditional "Regenerate flake.lock if inputs change" → inputs did NOT
  change → DO NOTHING. Do not run `nix flake update` / `nix flake lock`.

### CRITICAL: Validation is EVAL-ONLY — do NOT run `nix build`
`release.yml` (lines 308–328) + `flake.nix` STEP-1 comment: the flake ships with
`cargoHash = pkgs.lib.fakeHash` (deliberate placeholder) because qmk-notifier is a
git dep and Cargo.lock carries no vendor hash. A real `nix build .#qmkonnect` FAILS
with a fixed-output hash mismatch until a human runs the one-time hash-iteration.
That follow-up is a SEPARATE tracked task, NOT this one.
=> The only valid CI-style gate is `nix flake check --no-build` (evaluates every
output for BOTH x86_64-linux and aarch64-linux without instantiating a build).

### The NixOS module (nixosModule) — NO CHANGES NEEDED
It already: `environment.systemPackages = [ pkg ]` (PATH), `services.udev.packages = [ pkg ]`,
`systemd.packages = [ pkg ]`. On NixOS systemd is the PRIMARY autostart (udev
SYSTEMD_USER_WANTS + user unit); the `.desktop` is belt-and-suspenders. Adding the
`.desktop` to the package's `$out/etc/xdg/autostart/` makes it available but requires
no module wiring.

## Validation plan
1. `nix flake check --no-build` — must pass (eval both arches). If nix not
   installed, `nixfmt flake.nix` for syntax, or `nix-instantiate --parse flake.nix`.
2. Confirm the new step is syntactically inside the `postInstall = '' ... ''` block
   (before the closing `''`).
3. Grep-verify the dest path and source-ref spelling.

## README edit (`packaging/nix/README.md`)
- Intro paragraph currently: "ships the static udev rule + systemd user service".
  Update to also mention the XDG autostart `.desktop`.
- Optionally add a one-liner under the "Non-NixOS" section noting the `.desktop`
  is now in the package (for users who want login-autostart on non-systemd distros).

## Dependency / scope notes
- INPUTS already exist & Complete: `.desktop` (P2.M6.T1.S1 ✓), `flake.nix` (P1.M1.T2.S2 ✓).
- This is a 1-point leaf task. Pure additive edit to postInstall + a README note.
- Does NOT touch: PRD, tasks.json, any Rust source, the NixOS module, flake inputs,
  flake.lock, cargoHash.