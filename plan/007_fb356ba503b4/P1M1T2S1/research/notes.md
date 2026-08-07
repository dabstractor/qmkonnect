# Research Notes — P1.M1.T2.S1: create `flake.nix` (buildRustPackage + Linux deps)

Repo: **`/home/dustin/projects/qmkonnect`**. New files: `flake.nix`, `flake.lock`
(repo root), `packaging/nix/README.md` (Mode-A docs). **No Rust/source change.**

## 0. Scope boundary vs siblings (no overlap)

- **S1 (this task):** `flake.nix` + `flake.lock` + `packaging/nix/README.md`. The
  flake's package builds the **binaries** (`qmkonnect` + `qmkonnect-hid-id`) via
  `buildRustPackage` (both `[[bin]]` targets build automatically into `$out/bin`).
- **P1.M1.T2.S2 (sibling, planned):** "Add udev rule + hid-id + systemd integration
  to Nix package" — wires the static udev rule + the hid-id helper placement + the
  systemd template INTO the package (postInstall / passthru). **NOT this task.**
  My flake.nix must NOT add that integration; it ships binaries only.
- **P1.M1.T1.S2 (parallel, AUR):** `packaging/linux/aur/` files — different dir,
  no overlap. (Confirmed by reading its PRP: it owns AUR `.SRCINFO`/`publish.sh`.)
- My README **documents** the post-install udev/systemd steps a USER runs manually
  (Nix can't install udev rules system-wide) — this is user-facing prose, distinct
  from S2's package integration. I point to the existing Linux docs rather than
  re-deriving them.

## 1. Codebase facts that drive the flake

From `Cargo.toml`:
- `name = "qmkonnect"`, `version = "0.2.8"`, `edition = "2021"`, `rust-version =
  "1.88"`, `license = "MIT"`, `publish = false`.
- **Two `[[bin]]` targets**: `qmkonnect` (`src/main.rs`) + `qmkonnect-hid-id`
  (`src/bin/hid_id.rs`). `buildRustPackage` builds BOTH automatically.
- `default = ["hyprland", "macos", "linux-tray"]`. The `macos` feature is an
  inert no-op on Linux (PRD §PACKAGING 2), so default-features build is the full
  Linux app (Hyprland + SNI tray). **Use default features** (don't pass
  `--no-default-features` — that'd drop the tray).
- **GIT dependency**: `qmk-notifier = { git = "https://github.com/dabstractor/qmk-notifier",
  tag = "v0.3.0" }`. This is the #1 Nix wrinkle (see §3).
- Linux target deps (from `[target.'cfg(target_os="linux")']`): `hyprland` (opt),
  `libxdo`, `tempfile`, `libc`, `ksni` (opt), `gtk` 0.18 (opt).
- `[profile.release]`: `opt-level="z"`, `lto=true`, `codegen-units=1`,
  `panic="abort"`, `strip=true`. `buildRustPackage` honors `[profile.release]`
  from Cargo.toml automatically — no flake-side replication needed.
- `.cargo/config.toml`: Windows-only `crt-static` rustflags → **no-op on Linux**;
  `buildRustPackage` reads it but it's gated to `windows-msvc`. Harmless.

From `Cargo.lock`: present (89 KB, 360 packages). Used for reproducible builds.

From the PKGBUILD (the build-parity reference): `build()` runs
`RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release` and builds BOTH
bins. `depends=('systemd' 'hidapi' 'libusb' 'zenity' 'libnotify')`;
`makedepends=('cargo' 'rust' 'libx11' 'libxcb' 'systemd-libs' 'pkg-config')`.

## 2. The buildInputs mapping (Ubuntu CI → Nixpkgs)

The CI linux-binary job's apt deps → Nixpkgs package names (the item gives the
target list; verified against the crate's actual link needs):

| Ubuntu apt dep                | Nixpkgs package             | why                                  |
|-------------------------------|-----------------------------|--------------------------------------|
| libgtk-3-dev                  | `gtk3`                      | `gtk` 0.18 crate (linux-tray feat)   |
| libglib2.0-dev                | `glib`                      | gtk transitive                       |
| libayatana-appindicator3-dev  | `libayatana-appindicator`   | tray-icon native hint                |
| libx11-dev                    | `xorg.libX11` (or `libx11`) | X11 monitor + libxdo                 |
| libxcb1-dev                   | `xorg.libxcb` (or `libxcb`) | X11 monitor                          |
| libxdo-dev                    | `xdotool`                   | `libxdo` crate → libxdo.so           |
| libhidapi-dev                 | `hidapi`                    | `hidapi` crate                       |
| libudev-dev                   | `systemd` (or `systemdMinimal`) | `libc` geteuid + hidapi hidraw    |
| libusb                        | `libusb`                    | hidapi libusb backend fallback       |
+ `nativeBuildInputs = [ pkg-config ]` (the crate probes hidapi/gtk via pkg-config).

The item's list `[ gtk3 glib libayatana-appindicator libx11 libxcb xdotool hidapi
libusb systemd ]` is the target; I use it verbatim (note: `libx11`/`libxcb` in
nixpkgs are aliases to the xorg.* libs; both spellings resolve).

## 3. The git-dependency wrinkle (#1 Nix gotcha)

`qmk-notifier` is a **git** dependency. Per nixpkgs Rust docs
(ryantm.github.io/nixpkgs/languages-frameworks/rust) + GitHub nixpkgs#183344:
- `Cargo.lock` does NOT carry the output hash of git deps.
- Two resolution paths, BOTH require one hash-iteration build:
  - **`cargoHash = lib.fakeHash;`** (RECOMMENDED): buildRustPackage's cargo-fetch
    phase vendors ALL deps (crates + the git dep) into one fixed-output dir;
    `lib.fakeHash` (`sha256-AAAA…=`) fails the first build and prints the real
    `got: sha256-…`. Paste it. ONE hash covers everything. **This is the path I
    recommend** — simplest, handles the git dep transparently.
  - `cargoLock = { lockFile = ./Cargo.lock; outputHashes = { "qmk-notifier-0.3.0"
    = "<hash>"; }; }` — reads crates from Cargo.lock but still needs the git dep's
    `outputHashes` entry (also a fakeHash→real iteration).
- **I CANNOT pre-compute the hash here** — `nix` is NOT installed in this env
  (verified: `which nix` → none). The PRP's flake.nix uses `lib.fakeHash` and the
  runbook makes the iteration step explicit + mandatory (it's the standard Nix
  workflow, well-documented).

## 4. The `-lhidapi-hidraw` linker flag (#2 Nix gotcha — may need a fallback)

The PKGBUILD + external_deps.md §5 mandate `RUSTFLAGS="-C link-arg=-lhidapi-hidraw"`
(parity: hidraw backend so usage/usage_page matching works, not libusb). This works
on Arch/Ubuntu because they ship the SPLIT `libhidapi-hidraw.so` (hidapi <0.14 layout;
Debian even ships separate `libhidapi-hidraw0`/`libhidapi-libusb0` packages).

**Risk:** hidapi upstream ≥0.14 UNIFIED the backends into one `libhidapi.so`
(selected at runtime). Nixpkgs tracks upstream closely → current Nixpkgs `hidapi`
is very likely the unified build, which does NOT ship `libhidapi-hidraw.so` →
`-lhidapi-hidraw` fails with `cannot find -lhidapi-hidraw: No such file or directory`.

**Resolution (documented in the PRP; surfaces in the SAME first build as the cargoHash
iteration, so the implementer resolves both together):**
- **Primary (contract-faithful):** keep `RUSTFLAGS = "-C link-arg=-lhidapi-hidraw"` +
  `hidapi` in buildInputs (the item/external_deps mandate it).
- **Fallback (if the first build reports `cannot find -lhidapi-hidraw`):** the Nixpkgs
  hidapi is unified. The unified `libhidapi.so` auto-selects the **hidraw** backend on
  Linux at runtime (that's the whole point of unification), so usage/usage_page
  matching STILL works. Fix = drop the `-C link-arg=-lhidapi-hidraw` from RUSTFLAGS
  (let the `hidapi` crate link the unified `libhidapi.so` via pkg-config). The PRP
  spells this out as a diagnosed one-line fix keyed to the exact error message.

I cannot test which case holds (no Nix here). The PRP presents the contract version
as primary + a deterministic, error-message-keyed fallback. This is the responsible
guidance given the env constraint.

## 5. Other Nix design decisions

- **System set:** `packages.x86_64-linux` + `packages.aarch64-linux` (item mandate).
  Use `flake-utils.lib.eachSystem ["x86_64-linux" "aarch64-linux"]` (NOT
  `eachDefaultSystem` — that'd try to build gtk3/hidapi on Darwin and fail). macOS/
  Windows have their own native installers (F8); the Nix flake is the Linux F15 channel.
- **`doCheck = false;`** — the bin's tests need `--test-threads=1` (shared global
  debouncer) + HID hardware; running them in the Nix build is non-deterministic. Skip
  the check phase; the package just compiles the binaries. (CI runs tests separately.)
- **`default` = qmkonnect** + `mainProgram = "qmkonnect"` (enables `nix run`).
- **`devShells.default`** = `mkShell` with the SAME nativeBuildInputs/buildInputs +
  cargo/rustc/clippy/rustfmt + the RUSTFLAGS, so `nix develop` gives a buildable shell.
- **`flake.lock`**: generated by `nix flake lock` (implementer runs it; I can't
  pre-generate without nix). It pins nixpkgs + flake-utils revisions.
- **`profile.release`**: honored from Cargo.toml automatically — do NOT replicate
  `opt-level`/`lto`/`strip` in the flake.
- **The hid-id bin**: builds automatically (it's a `[[bin]]`); it lands in `$out/bin/
  qmkonnect-hid-id`. S2 will relocate it + add the udev rule; this task ships it as-is.

## 6. Validation reality (env-gated)

- **Nix is NOT installed in this research env** (`which nix` → none). I CANNOT run
  `nix build` / `nix flake lock` / `nix flake check` here to pre-validate.
- The PRP's validation gate is therefore **run by the implementer in a Nix env**:
  `nix flake lock` → `nix build .#qmkonnect` (iterate cargoHash; resolve hidapi link
  if needed) → `nix run .#qmkonnect -- --version` → `nix flake check`.
- I CAN statically sanity-check the Nix expression syntax only conceptually (no `nix
  eval` here). The flake.nix I provide is idiomatic and follows current buildRustPackage
  conventions; the two iteration steps (cargoHash, hidapi link) are the only
  env-dependent resolutions, and both are standard Nix workflows with clear error
  messages.
- This env constraint is stated HONESTLY in the PRP (it's a quality-gate requirement:
  validation commands must be "verified working" — I verify the COMMANDS are correct
  and the env requirement is explicit, rather than claiming a pass I didn't run).

## 7. Mode-A docs deliverable (packaging/nix/README.md)

Contents (user-facing; points to existing Linux docs rather than re-deriving):
1. **Install**: `nix profile install github:dabstractor/qmkonnect` (and `nix run` /
   `nix build` alternatives).
2. **Post-install udev + systemd (manual)**: Nix is per-user and CANNOT install udev
   rules system-wide. The user must, once: copy/symlink the static rule
   (`packaging/linux/udev/69-qmkonnect-rawhid.rules`) to `/usr/lib/udev/rules.d/`,
   place the `qmkonnect-hid-id` helper where the rule's IMPORT expects it (or adjust
   the path), `udevadm control --reload && udevadm trigger`, and (optionally)
   instantiate the systemd user service template. **Cross-ref** `docs/installation.md`
   Linux section + `spec/LINUX.md` §6 for the exact commands — do NOT duplicate them.
3. **`nix develop`**: drop into a shell with cargo + all build deps pre-installed
   (the devShell) for hacking on the crate without polluting the host.
4. **Note the boundary**: the flake ships the binaries; the system-wide udev/systemd
   bits are a documented manual post-install (and S2 will make the package SHIP the
   rule/helper/template for users who want them wired automatically).

## 8. External sources consulted

- nixpkgs Rust manual: https://ryantm.github.io/nixpkgs/languages-frameworks/rust/
  (buildRustPackage, cargoHash vs cargoLock, git-dep outputHashes).
- nixpkgs#183344: cargoLock requires outputHashes for git deps.
- "Nix: buildRustPackage with git deps" (artemis.sh): the outputHashes pattern.
- hidapi upstream README (libusb/hidapi): split `libhidapi-hidraw`/`libhidapi-libusb`
  for hidapi <0.14; unified `libhidapi.so` for ≥0.14 (backend selected at link/runtime).
- hidapi-rs build.rs (crates.io hidapi 2.6): feature-gated Linux backends.
- The project's own PKGBUILD (build-parity reference) + external_deps.md §5.