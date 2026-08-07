# PRP — P1.M1.T2.S2: udev rule + hid-id + systemd integration in the Nix package

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Modifies 2 files:**
> `flake.nix` (add a `postInstall` that ships the udev rule + hid-id helper +
> systemd user service from the package, rewritten to Nix store paths; restructure
> `outputs` to add a `nixosModules.default` NixOS module) + `packaging/nix/README.md`
> (add the NixOS-module usage section; relabel the manual section for non-NixOS).
> **No Rust/source change** — pure Nix packaging + Mode-A docs. The crate's udev
> rule + service template **source files stay FHS-pathed** (correct for Arch/Ubuntu);
> the FHS→store-path rewrite happens at BUILD time in `postInstall`.
> **⚠ Env-gated validation.** `nix` is NOT installed in the authoring env. The
> implementer runs the gate (`nix build .#qmkonnect` → verify the 3 shipped files +
> their rewritten paths → `nix flake check`) in a Nix-capable env. `cargoHash` is
> still `pkgs.lib.fakeHash` on disk (S1's Nix-env iteration not yet run) — the
> `postInstall` only executes once `nix build` succeeds, so the cargoHash iteration
> is a precondition of this task's validation.
> **Boundary:** S1 shipped the **binaries** (`flake.nix` + README, both on disk);
> this task (S2) adds the **udev/hid-id/systemd integration + the NixOS module**.
> The AUR sibling (P1.M1.T1.S2) owns `packaging/linux/aur/`; no overlap.

---

## Goal

**Feature Goal**: Make the Nix package ship **all four** runtime artifacts
(`qmkonnect`, `qmkonnect-hid-id`, the static udev rule, the systemd user service)
with the two hardcoded FHS paths (`/usr/lib/udev/qmkonnect-hid-id` in the rule,
`/usr/bin/qmkonnect` in the service) rewritten to the package's Nix store path, and
expose a **`nixosModules.default`** NixOS module so a NixOS user enables everything
with `services.qmkonnect.enable = true` (udev rule registered, systemd user service
made available, optional `input`-group membership). Non-NixOS Nix users keep the
manual post-install path (documented).

**Deliverable** (2 modified files, 0 new files):
- `flake.nix` — `packages.qmkonnect` gains a `postInstall` (installs + rewrites the
  3 files); `outputs` is restructured to `let perSystem = (eachSystem …); in
  perSystem // { nixosModules.default = <module>; }` so the system-agnostic module
  sits alongside the per-system outputs.
- `packaging/nix/README.md` — a new **"NixOS (recommended)"** section
  (`inputs.qmkonnect.nixosModules.default` + `services.qmkonnect.enable = true`);
  the existing manual section is relabeled **"Non-NixOS (Nix on another distro)"**;
  the "future update" note is replaced with "shipped by the module/package".

**Success Definition** (env-gated): in a Nix env, `nix build .#qmkonnect` succeeds
(after the inherited cargoHash iteration) and produces, in `result/`: `bin/qmkonnect`,
`bin/qmkonnect-hid-id`, `lib/udev/qmkonnect-hid-id`, `lib/udev/rules.d/69-qmkonnect-rawhid.rules`
(with `/usr/lib/udev/qmkonnect-hid-id` rewritten to the store path),
`lib/systemd/user/qmkonnect.service` (with `ExecStart=` rewritten to the store
path). `nix flake check` passes (evals `nixosModules.default` too). The README
documents both the NixOS one-line enablement and the non-NixOS manual path. No
source file (`Cargo.toml`, `src/`, the udev/systemd SOURCES, `docs/`, the Arch
PKGBUILD) is modified.

## User Persona (if applicable)

**Target User**: A NixOS user who wants QMKonnect's HID permissions + autostart
service set up **declaratively** (one line in `configuration.nix`), not by running
`sudo install` + `systemctl` manually each rebuild.

**Use Case**: User adds `inputs.qmkonnect.nixosModules.default` to their
`imports` and `services.qmkonnect.enable = true` to `configuration.nix`, rebuilds,
plugs in their QMK keyboard → the udev rule tags the device (permissions +
`qmkonnect_device` symlink), `SYSTEMD_USER_WANTS` auto-starts the user service,
keyboard lights up. Zero manual `sudo`/`systemctl` steps.

**Pain Points Addressed**: NixOS has no FHS (`/usr/lib/udev/`, `/usr/bin/` don't
exist), so the shipped udev rule + service template break verbatim. S1's README
told users to hand-copy the rule + helper + run `sudo udevadm` — friction on every
rebuild and a footgun (a stale helper after a flake update). This task makes the
package itself ship all three (rewritten to store paths) and wires them via a
NixOS module, so it's reproducible + automatic.

## Why

- **Closes the Nix integration (F15, Nix channel).** external_deps.md §5 mandates
  "Wrap udev rule installation in the package's setup hook or document manual
  steps." S1 documented the manual path; S2 delivers the package integration
  (the better half) + the NixOS module (the one-line path).
- **NixOS-correctness.** The two hardcoded FHS paths would silently break the rule
  (IMPORT fails → no device tagged) and the service (ExecStart → not found) on
  NixOS. Rewriting them to store paths at build time is the canonical Nix fix.
- **Unblocks P1.M5.T2.S2 (CI).** That task adds `nix flake check` to CI; this task
  ships the `nixosModules.default` output it will validate.
- **No Rust change.** The udev/systemd SOURCE files stay FHS-pathed (they're
  correct for Arch/Ubuntu/AUR); only the BUILT package's copies are rewritten.

## What

### The full updated `flake.nix` (verbatim — write the whole file)

The changes vs S1's on-disk version: (1) `outputs` body wrapped in
`let perSystem = (eachSystem …); in perSystem // { … }`; (2) the `packages.qmkonnect`
`buildRustPackage` gains a `postInstall`; (3) a new `nixosModules.default` attr.
Everything else (inputs, buildInputs, RUSTFLAGS, devShells, meta) is unchanged from S1.

```nix
{
  description = "Cross-platform window activity notifier for QMK keyboards";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    let
      # Per-system outputs (the package + the dev shell) — from S1 (P1.M1.T2.S1).
      # nixosModules (below) is system-agnostic, so it sits OUTSIDE eachSystem and
      # is merged in with `//` at the end.
      perSystem = flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
        let
          pkgs = import nixpkgs { inherit system; };

          # Build-time deps shared by the package + the dev shell.
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = with pkgs; [
            gtk3
            glib
            libayatana-appindicator
            libx11
            libxcb
            xdotool       # provides libxdo.so for the `libxdo` crate
            hidapi
            libusb
            systemd       # libudev (hidapi hidraw) + geteuid()
          ];

          # Parity with the Arch PKGBUILD: link the hidraw backend so usage/
          # usage_page device matching works (NOT the libusb backend). If the first
          # `nix build` fails with "cannot find -lhidapi-hidraw" your Nixpkgs ships
          # the UNIFIED hidapi (>=0.14) — see packaging/nix/README.md "hidapi link"
          # note + the fallback (drop the flag; unified libhidapi auto-selects
          # hidraw on Linux, so matching still works).
          hidrawFlag = "-C link-arg=-lhidapi-hidraw";
        in
        {
          packages.default = self.packages.${system}.qmkonnect;

          packages.qmkonnect = pkgs.rustPlatform.buildRustPackage {
            pname = "qmkonnect";
            # version is derived from Cargo.toml (buildRustPackage reads it).
            src = ./.;

            # STEP 1 of the cargoHash iteration: start with the fake hash, run
            # `nix build .#qmkonnect`, read the "got: sha256-…" from the failure,
            # paste it here, rebuild. (Required because Cargo.lock does not carry
            # the hash of the qmk-notifier GIT dependency.)
            cargoHash = pkgs.lib.fakeHash;

            inherit nativeBuildInputs buildInputs;
            RUSTFLAGS = hidrawFlag;

            # The bin's tests need --test-threads=1 (shared global debouncer) +
            # HID hardware; non-deterministic in a Nix build. CI runs them.
            doCheck = false;

            # Ship the udev rule + hid-id helper + systemd user service ALONGSIDE
            # the two binaries. NixOS has no FHS, so the rule's hardcoded
            # /usr/lib/udev/qmkonnect-hid-id and the service's /usr/bin/qmkonnect
            # are rewritten to this package's Nix store path ($out/...). The
            # NixOS module (nixosModules.default) registers the rule + service;
            # non-NixOS users do the manual install (see packaging/nix/README.md).
            postInstall = ''
              # 1. udev helper — the rule IMPORTs it by absolute path.
              install -Dm755 $out/bin/qmkonnect-hid-id $out/lib/udev/qmkonnect-hid-id

              # 2. static udev rule — rewrite the hardcoded helper path to the
              #    Nix store path so udev can run it at device-add time.
              install -Dm644 ${./packaging/linux/udev/69-qmkonnect-rawhid.rules} \
                $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules
              substituteInPlace $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules \
                --replace "/usr/lib/udev/qmkonnect-hid-id" "$out/lib/udev/qmkonnect-hid-id"

              # 3. systemd user service — instantiate .template -> .service and
              #    rewrite the hardcoded ExecStart to the Nix store path.
              substitute ${./packaging/linux/systemd/qmkonnect.service.template} \
                $out/lib/systemd/user/qmkonnect.service \
                --replace "/usr/bin/qmkonnect" "$out/bin/qmkonnect"
            '';

            meta = with pkgs.lib; {
              description = "Cross-platform window activity notifier for QMK keyboards";
              homepage = "https://github.com/dabstractor/qmkonnect";
              license = licenses.mit;
              mainProgram = "qmkonnect";
              platforms = [ "x86_64-linux" "aarch64-linux" ];
            };
          };

          devShells.default = pkgs.mkShell {
            inherit nativeBuildInputs buildInputs;
            packages = with pkgs; [ cargo rustc clippy rustfmt ];
            RUSTFLAGS = hidrawFlag;
          };
        });

      # The NixOS module — system-agnostic. Reference's this flake's own package
      # via self.packages.${pkgs.system}.qmkonnect (the postInstall above means it
      # ships the rule + helper + service). Import via
      # inputs.qmkonnect.nixosModules.default in a NixOS configuration.nix; enable
      # with services.qmkonnect.enable = true.
      nixosModule = { config, lib, pkgs, ... }:
        let
          cfg = config.services.qmkonnect;
          pkg = self.packages.${pkgs.system}.qmkonnect;
        in
        {
          options.services.qmkonnect = {
            enable = lib.mkEnableOption (lib.mdDoc "QMKonnect — udev rule + systemd user service for QMK keyboards");

            user = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = lib.mdDoc ''
                Username to add to the `input` group (the udev rule's GROUP="input"
                fallback). Optional: the rule's `uaccess` tag grants the active
                logged-in user access via systemd-logind, so this is only needed
                if you rely on the raw group permission instead of the session
                ACL. The named user must be defined elsewhere in your configuration.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            # Put the qmkonnect binary on PATH (for manual runs / the tray /
            # `qmkonnect --list-callbacks`). The systemd service uses its
            # store-path ExecStart, so it doesn't depend on PATH, but the
            # user-facing binary should be runnable when the module is enabled.
            environment.systemPackages = [ pkg ];

            # 1. Static udev rule: tags QMK Raw HID devices, sets GROUP/MODE +
            #    uaccess, creates the qmkonnect_device symlink, and
            #    SYSTEMD_USER_WANTS the service on plug. The rule's IMPORT runs the
            #    helper at its (postInstall-rewritten) Nix store path.
            services.udev.packages = [ pkg ];

            # 2. systemd user service: makes the unit available in the system-wide
            #    user unit library (/etc/systemd/user/). The udev rule's
            #    SYSTEMD_USER_WANTS auto-starts it on device plug; for login-time
            #    start the user runs `systemctl --user enable --now qmkonnect`.
            systemd.packages = [ pkg ];

            # 3. Optional: add a user to the `input` group (the GROUP fallback;
            #    uaccess is primary, so this is opt-in).
            users.users = lib.optionalAttrs (cfg.user != null) {
              ${cfg.user}.extraGroups = [ "input" ];
            };
          };
        };
    in
    perSystem // {
      nixosModules.default = nixosModule;
    };
}
```

> **Edit note for the implementer:** the cleanest way to apply this is to
> **rewrite `flake.nix` wholesale** with the block above (it preserves every S1
> byte except the three additions). Do NOT try to surgically edit the `outputs = …
> flake-utils.lib.eachSystem …` opening into the `let perSystem = … in … // …`
> form via `edit` oldText/newText — the wrapping change spans the whole `outputs`
> body and is error-prone; a full-file `write` is safer and the diff is reviewable.

### The updated `packaging/nix/README.md` (verbatim — write the whole file)

S1's content with: a new **"NixOS (recommended)"** section inserted after the
"Install" section; the old "Post-install: udev rule + systemd service (one-time,
manual)" section **relabeled** "Non-NixOS (Nix on another distro)"; the
"(A future update will ship …)" sentence replaced.

```markdown
# QMKonnect — Nix flake

QMKonnect ships a [Nix flake](https://nixos.wiki/wiki/Flakes) that builds the app
**from source** against pinned [Nixpkgs](https://github.com/NixOS/nixpkgs), with
all Linux system dependencies (GTK3, hidapi, libxdo, …) provided automatically.

The flake builds both binaries — `qmkonnect` (the app) and `qmkonnect-hid-id`
(the udev helper) — for `x86_64-linux` and `aarch64-linux`, and also ships the
static udev rule + systemd user service from the package (rewritten to the Nix
store path) plus a `nixosModules.default` NixOS module for one-line enablement.

## Install

```sh
# Add to your user profile (recommended for non-NixOS):
nix profile install github:dabstractor/qmkonnect

# …or run ad-hoc without installing:
nix run github:dabstractor/qmkonnect

# …or build into ./result without installing:
nix build github:dabstractor/qmkonnect
# binary: ./result/bin/qmkonnect   (helper: ./result/bin/qmkonnect-hid-id)
```

## NixOS (recommended)

On NixOS, import the flake's `nixosModules.default` and enable the service in your
`configuration.nix` (or a flake-based `nixosConfigurations` host). The module
registers the static udev rule (`services.udev.packages`), makes the systemd user
service available (`systemd.packages`), and puts `qmkonnect` on `PATH`. The udev
rule's `SYSTEMD_USER_WANTS` then auto-starts the user service when your keyboard
plugs in.

```nix
# flake.nix (yours)
{
  inputs.qmkonnect.url = "github:dabstractor/qmkonnect";
  outputs = { self, nixpkgs, qmkonnect, ... }: {
    nixosConfigurations.myhost = nixpkgs.lib.nixosSystem {
      system = "x86_64-linux";
      modules = [
        ./configuration.nix
        qmkonnect.nixosModules.default   # <-- the module
      ];
    };
  };
}
```

```nix
# configuration.nix (yours)
{ ... }: {
  services.qmkonnect.enable = true;          # udev rule + systemd user service + PATH

  # Optional: the udev rule sets GROUP="input" as a fallback (the `uaccess` tag is
  # primary and handles a normal logged-in desktop session). Add yourself to
  # `input` only if you rely on the raw group permission instead of the session
  # ACL. The named user must be defined elsewhere in your config.
  # services.qmkonnect.user = "your-username";

  # (Define your user elsewhere, e.g.:)
  # users.users.your-username = { isNormalUser = true; ... };
}
```

After `nixos-rebuild switch`, plug in your QMK keyboard — the udev rule tags the
device (permissions + a `qmkonnect_device` symlink) and starts the user service.
To also start it at every login (not just on plug), run once:
`systemctl --user enable --now qmkonnect.service`.

## Non-NixOS (Nix on another distro)

If you run Nix on Arch/Ubuntu/Fedora/… (not NixOS), the package can't install
udev rules system-wide for you, so the HID permissions + optional autostart
service are a one-time manual step (identical to the generic Linux install). From
the installed `qmkonnect-hid-id` (e.g. `~/.nix-profile/bin/qmkonnect-hid-id` after
`nix profile install`) and a checkout of the repo:

```sh
# 1. Static udev rule — IDENTICAL for every keyboard (no per-VID/PID config).
#    Rewrite the IMPORT helper path to wherever you install the helper below.
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules \
  /usr/lib/udev/rules.d/
sudo sed -i "s#/usr/lib/udev/qmkonnect-hid-id#$(command -v qmkonnect-hid-id)#" \
  /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules

# 2. The udev helper the rule IMPORTs. (The package ships it; symlink or copy it
#    to the path the rule now references — command -v resolves the profile path.)
#    It tags any hidraw interface carrying the QMK Raw HID signature.
sudo ln -sf "$(command -v qmkonnect-hid-id)" /usr/lib/udev/qmkonnect-hid-id

# 3. Reload udev so the rule + permissions take effect.
sudo udevadm control --reload-rules && sudo udevadm trigger
```

For the optional systemd user service (autostart at login), instantiate
`packaging/linux/systemd/qmkonnect.service.template` with the binary path, as in
the **Linux install** section of [docs/installation.md](../../docs/installation.md)
and `spec/LINUX.md §6` — substitute `/usr/bin/qmkonnect` for `$(command -v
qmkonnect)` in `ExecStart`.

## `nix develop` — contribute without polluting your host

```sh
nix develop github:dabstractor/qmkonnect
# → a shell with cargo, rustc, clippy, rustfmt, AND every system lib the crate
#   needs (gtk3, hidapi, libxdo, …) already on the include/link paths.
cargo build --release     # just works — no apt/pacman dance
cargo test --bin qmkonnect -- --test-threads=1
```

## hidapi link note (build-parity with the Arch PKGBUILD)

The flake links the **hidraw** HID backend (`RUSTFLAGS="-C link-arg=-lhidapi-hidraw"`)
so device matching works by **usage page/usage**, not VID/PID — the same choice the
Arch package makes. If your Nixpkgs revision ships the *unified* hidapi (≥0.14,
which folds both backends into one `libhidapi.so`) and the build fails with
`cannot find -lhidapi-hidraw`, remove the `hidrawFlag` line in `flake.nix`: the
unified `libhidapi.so` auto-selects the hidraw backend on Linux at runtime, so
usage/usage_page matching still works.
```

### Success Criteria

- [ ] `flake.nix` `packages.qmkonnect` has a `postInstall` that installs the helper
      to `$out/lib/udev/`, the rule to `$out/lib/udev/rules.d/` (with the helper
      path substituted), and the service to `$out/lib/systemd/user/` (instantiated
      from `.template`, with ExecStart substituted).
- [ ] `flake.nix` `outputs` is `let perSystem = (eachSystem …); in perSystem // {
      nixosModules.default = <module>; }`.
- [ ] `nixosModules.default` is a NixOS module with `options.services.qmkonnect.
      {enable, user}` and `config = mkIf cfg.enable { services.udev.packages;
      systemd.packages; environment.systemPackages; optional input-group }`.
- [ ] `packaging/nix/README.md` has a "NixOS (recommended)" section (the module
      usage) + a relabeled "Non-NixOS" manual section; the "future update" note is
      gone.
- [ ] (Nix env) `nix build .#qmkonnect` → the 5 files exist in `result/` (2 bins +
      helper + rule + service); the rule's helper path + the service's ExecStart
      are the Nix store path (no `/usr/lib/udev/`/`/usr/bin/`).
- [ ] (Nix env) `nix flake check` passes (evals `nixosModules.default`).
- [ ] No source file modified (`Cargo.toml`, `src/`, the udev/systemd SOURCES,
      `docs/`, the Arch PKGBUILD).

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?"_ — **Yes.** The full verbatim `flake.nix` (with
> the postInstall + the restructured outputs + the module) and the full verbatim
> README are below; the two FHS-path-rewrite gotchas (the rule's IMPORT helper path,
> the service's ExecStart) are spelled out with the exact `substitute`/`substituteInPlace`
> calls; the NixOS module's three mechanisms (`services.udev.packages`,
> `systemd.packages`, the optional `input`-group option) and WHY each is correct on
> NixOS are documented; the env-gate + the inherited cargoHash precondition are
> stated. The implementer's only judgment call (in a Nix env) is confirming
> `systemd.packages` links the user unit (if not, the fallback is noted).

> **PARALLEL-SIBLING NOTE.** S1 (`flake.nix` + README) is being implemented in
> parallel and is ALREADY on disk (verified). This task EDITS those 2 files. There
> is no collision with the AUR sibling (`packaging/linux/aur/`).

### Documentation & References

```yaml
# MUST READ — the S1 contract (the flake.nix + README this task extends)
- file: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/P1M1T2S1/PRP.md
  why: "S1's verbatim flake.nix (eachSystem, buildRustPackage, the 9 buildInputs,
        RUSTFLAGS=hidrawFlag, doCheck=false, meta with mainProgram) + its verbatim
        README (install + manual post-install + nix develop + hidapi note). S1
        explicitly DEFERS the udev/hid-id/systemd package integration to 'S2
        (sibling)'. This task IS that S2. The on-disk flake.nix/README already
        match S1's PRP — verify before editing."
  section: "What (flake.nix), What (README), Anti-Patterns (don't add postInstall in S1)"
  critical: "cargoHash is STILL pkgs.lib.fakeHash on disk (S1's Nix-env iteration
             not run). postInstall only runs once `nix build` succeeds, so the
             cargoHash iteration is a PRECONDITION of this task's validation."

# MUST READ — the two source files whose FHS paths get rewritten at build time
- file: /home/dustin/projects/qmkonnect/packaging/linux/udev/69-qmkonnect-rawhid.rules
  why: "The rule text. The IMPORT{program} line hardcodes /usr/lib/udev/qmkonnect-hid-id
        — that exact string is what postInstall's substituteInPlace --replace targets.
        The rest (GROUP/MODE/uaccess/SYMLINK/SYSTEMD_USER_WANTS) is FHS-independent
        and needs NO rewrite. DO NOT edit this source file — only the built copy."
  pattern: "substituteInPlace <out-rule> --replace '/usr/lib/udev/qmkonnect-hid-id' '$out/lib/udev/qmkonnect-hid-id'"
- file: /home/dustin/projects/qmkonnect/packaging/linux/systemd/qmkonnect.service.template
  why: "The service template. ExecStart=/usr/bin/qmkonnect is the FHS path to
        rewrite. BindsTo=dev-qmkonnect_device.device names the DEVICE UNIT from the
        udev-rule SYMLINK (qmkonnect_device) — FHS-independent, NO rewrite. The
        postInstall substitutes + instantiates .template -> .service in one
        `substitute` call."
  pattern: "substitute <template> <out-service> --replace '/usr/bin/qmkonnect' '$out/bin/qmkonnect'"

# MUST READ — the canonical NixOS patterns the module uses
- url: https://wiki.nixos.org/wiki/Systemd/User_Services
  why: "Confirms NixOS installs user unit files into /etc/systemd/user/ (the
        system-wide user-unit library) so every user's user-manager can find them.
        systemd.packages = [ pkg ] is the option that links a package's
        lib/systemd/user/*.service there. The udev rule's SYSTEMD_USER_WANTS then
        auto-starts the unit on device plug."
  critical: "NixOS base has NO declarative systemd.user.services.<name> (that's
             home-manager). 'Make available via systemd.packages + rely on the
             udev SYSTEMD_USER_WANTS / a one-time systemctl --user enable' is the
             correct base-NixOS ceiling."
- url: https://github.com/NixOS/nixpkgs/blob/master/nixos/modules/services/hardware/udev.nix
  why: "The services.udev.packages option: adds a package's lib/udev/rules.d/*.rules
        to the system udev rules AND realizes the package (so the rule's IMPORT can
        run the helper at its store path). This is how every NixOS udev-rule-shipping
        module works (yubikey, openrazer, …)."

# REFERENCE — the build-parity reference (the Arch PKGBUILD does the same install)
- file: /home/dustin/projects/qmkonnect/packaging/linux/arch/PKGBUILD
  why: "package() installs the helper to /usr/lib/udev/qmkonnect-hid-id, the rule
        to /usr/lib/udev/rules.d/, the template to /usr/lib/systemd/user/; the
        .install post_install copies .template -> .service + systemctl --global
        enable. The Nix postInstall mirrors the file placement (to $out/...) and
        folds the instantiation into the substitute step (Nix has no post_install
        hook; the module replaces 'systemctl --global enable')."
  section: "package() + the .install post_install"

# REFERENCE — research notes (verified facts + the 2 FHS-path gotchas + the env gate)
- docfile: /home/dustin/projects/qmkonnect/plan/007_fb356ba503b4/P1M1T2S2/research/notes.md
  why: "The on-disk S1 state, the two hardcoded-FHS-path gotchas, the postInstall
        idiom (install/substitute/substituteInPlace), the nixosModules restructure
        (perSystem // {…}), the three module mechanisms + WHY each is correct on
        NixOS, the README update plan, and the env-gated validation."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── flake.nix                          # <-- MODIFY (add postInstall + nixosModules; restructure outputs)
├── packaging/
│   ├── nix/README.md                  # <-- MODIFY (add NixOS section; relabel manual)
│   ├── linux/udev/69-qmkonnect-rawhid.rules        # SOURCE (FHS path; rewritten at build) — DO NOT EDIT
│   ├── linux/systemd/qmkonnect.service.template    # SOURCE (FHS path; rewritten at build) — DO NOT EDIT
│   └── linux/arch/PKGBUILD            # build-parity reference — DO NOT EDIT
├── src/bin/hid_id.rs                  # the qmkonnect-hid-id [[bin]] — DO NOT EDIT
├── Cargo.toml                         # the 2 [[bin]] names — DO NOT EDIT
└── docs/installation.md               # README cross-refs its Linux section — DO NOT EDIT
```

### Desired Codebase tree with files to be modified

```bash
flake.nix                  # MODIFIED — postInstall + nixosModules.default + outputs restructure
packaging/nix/README.md    # MODIFIED — NixOS (recommended) section + relabeled non-NixOS manual
# (no new files; no source/udev/systemd-source/docs change)
```

### Known Gotchas of our codebase & Library Quirks

```nix
# CRITICAL: NixOS has NO FHS — /usr/lib/udev/ and /usr/bin/ do not exist. The
#   shipped udev rule (IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id …") and
#   the service template (ExecStart=/usr/bin/qmkonnect) break verbatim on NixOS
#   (IMPORT silently fails -> no device tagged; ExecStart -> not found). The
#   postInstall MUST rewrite BOTH to $out/... (the package's store path). The
#   SOURCE files stay FHS-pathed (correct for Arch/Ubuntu); only the BUILT
#   copies are rewritten.

# CRITICAL: BindsTo=dev-qmkonnect_device.device needs NO rewrite. It names the
#   device unit derived from the qmkonnect_device SYMLINK the (rewritten) udev
#   rule creates — the symlink name is FHS-independent and works on NixOS. Don't
#   "fix" it; you'd break the device-binding.

# CRITICAL: nixosModules is SYSTEM-AGNOSTIC — it must be a SIBLING of the
#   eachSystem block in outputs, NOT inside it. The structure is
#   `let perSystem = (eachSystem …); in perSystem // { nixosModules.default = …; }`.
#   Putting nixosModules inside eachSystem would make it per-system (wrong) and
#   break `inputs.qmkonnect.nixosModules.default` import.

# CRITICAL: the module references the flake's own package via
#   self.packages.${pkgs.system}.qmkonnect. `self` is captured by the outputs
#   closure (it's a param of outputs); `pkgs.system` is the host system in the
#   module. This is the canonical flake-ships-package-AND-module pattern. No
#   cycle: the package doesn't depend on the module.

# CRITICAL: cargoHash is STILL pkgs.lib.fakeHash on disk (S1's iteration not run
#   in the authoring env). postInstall only runs once `nix build .#qmkonnect`
#   SUCCEEDS, which requires the real cargoHash. So this task's validation
#   INCLUDES the inherited cargoHash iteration (fakeHash -> read "got:" -> paste).
#   The two iterations are independent: cargoHash unblocks the build; postInstall
#   runs after.

# GOTCHA: NixOS base has NO declarative systemd.user.services.<name> option (that
#   is home-manager). The module "enables" the user service by making it AVAILABLE
#   (systemd.packages -> /etc/systemd/user/) and relying on the udev rule's
#   SYSTEMD_USER_WANTS to auto-start it on plug (+ the user can `systemctl --user
#   enable` for login-time start). Do NOT try to add a `systemd.user.services`
#   option — it doesn't exist in base NixOS.

# GOTCHA: `uaccess` is the PRIMARY permission; `input` group is the FALLBACK
#   (PRD §2: uaccess can race logind on replug, so GROUP/MODE backs it up). On a
#   normal NixOS desktop with logind, uaccess suffices. So services.qmkonnect.user
#   (the input-group option) is OPTIONAL (default null) — don't force it.

# GOTCHA: `${./packaging/linux/...}` is a flake PATH LITERAL — its store path is
#   evaluated at flake-eval time (independent of src = ./.). This is the idiomatic
#   way to reference repo files in postInstall. (The file is also in $src, but
#   $src isn't reliably set in postInstall; the path literal is robust.)

# GOTCHA: `install -D` creates parent dirs; `-m755` for the executable helper,
#   `-m644` for the rule. `substituteInPlace` edits in place (after install);
#   `substitute` copies + replaces in one step (use it to instantiate the service).

# NOTE: the helper is COPIED (not symlinked) to $out/lib/udev/ — parity with the
#   Arch PKGBUILD (which install -Dm755s it) and avoids any udev IMPORT
#   symlink-resolution edge case. It's a tiny pure-std binary; the duplication is
#   negligible (Nix dedups store contents).

# NOTE: `nix` is NOT installed in the authoring env — validation is env-gated.
#   The implementer runs `nix build .#qmkonnect` (verify the 5 files + the
#   rewritten paths) + `nix flake check` (evals nixosModules.default) in a Nix env.
```

## Implementation Blueprint

### Data models and structure

No data models. The "structure" is the flake's `outputs` shape:
`let perSystem = (eachSystem […]); in perSystem // { nixosModules.default = <module>; }`,
where `packages.qmkonnect` carries a `postInstall` that ships + rewrites 3 files.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the on-disk S1 state + the 2 source files (anchor the edits)
  - READ: flake.nix (confirm it matches S1's PRP: eachSystem, buildRustPackage with
          fakeHash, no postInstall, no nixosModules), packaging/nix/README.md.
  - READ: packaging/linux/udev/69-qmkonnect-rawhid.rules (note the EXACT
          /usr/lib/udev/qmkonnect-hid-id string in the IMPORT line) +
          packaging/linux/systemd/qmkonnect.service.template (note ExecStart=/usr/bin/qmkonnect).
  - CONFIRM: Cargo.toml has [[bin]] qmkonnect + qmkonnect-hid-id (both build to
          $out/bin). GOAL: know the exact FHS strings to rewrite + the S1 structure.

Task 2: REWRITE flake.nix (verbatim from the "What" section)
  - WRITE: the full flake.nix block above (it preserves every S1 byte + adds the
          postInstall, the perSystem//{nixosModules.default} restructure, and the
          module). Use the `write` tool (a full-file rewrite is safer than surgical
          `edit` for the outputs-wrapping change).
  - CHECK: the postInstall's 3 steps are present (install helper; install+substitute
          rule; substitute service). The module has options + mkIf config with the
          4 config lines. nixosModules.default = nixosModule in the final `//`.

Task 3: ITERATE cargoHash (inherited from S1 — a precondition for postInstall)
  - RUN (Nix env): nix build .#qmkonnect
  - EXPECT: it FAILS with a fixed-output mismatch ("specified: sha256-AAAA…
          (lib.fakeHash)" / "got: sha256-<REAL>"). Paste the got: hash into
          cargoHash, rebuild. (Required because Cargo.lock has no hash for the
          qmk-notifier git dep — standard buildRustPackage workflow.)
  - This MUST succeed before postInstall runs (postInstall is part of the build).

Task 4: RESOLVE the hidapi link IF it fails (inherited from S1)
  - IF the (post-cargoHash) build fails with "cannot find -lhidapi-hidraw":
          Nixpkgs hidapi is the UNIFIED >=0.14 build. Set `RUSTFLAGS = "";` (or
          remove the line) in BOTH packages.qmkonnect and devShells.default.
          Rebuild. (Unified libhidapi auto-selects hidraw on Linux; matching works.)
  - DOCUMENT which path you took (so CI/S1 reconciliation knows).

Task 5: VERIFY the package output (Nix env — the postInstall ran)
  - RUN: nix build .#qmkonnect && find result/lib -type f
  - EXPECT: result/lib/udev/qmkonnect-hid-id, result/lib/udev/rules.d/69-qmkonnect-rawhid.rules,
          result/lib/systemd/user/qmkonnect.service. (Plus result/bin/qmkonnect +
          result/bin/qmkonnect-hid-id.)
  - RUN: grep -c '/usr/lib/udev/qmkonnect-hid-id' result/lib/udev/rules.d/69-qmkonnect-rawhid.rules
          -> Expected: 0 (rewritten). And `grep -o '/nix/store/[^ ]*/qmkonnect-hid-id'
          result/lib/udev/rules.d/*.rules` -> the store path.
  - RUN: grep '^ExecStart=' result/lib/systemd/user/qmkonnect.service
          -> Expected: ExecStart=/nix/store/.../bin/qmkonnect (NOT /usr/bin/qmkonnect).

Task 6: VERIFY the module evaluates (Nix env)
  - RUN: nix flake check
  - EXPECT: passes (evals packages per-system AND nixosModules.default). A Nix-expr
          error in the module (e.g. a typo in options/config, a missing lib.mdDoc)
          surfaces here.
  - (Optional, deeper) Build a minimal nixosConfigurations.test that imports
          nixosModules.default + sets services.qmkonnect.enable = true, then
          `nix run nixpkgs#nixos-rebuild -- build-vm --flake .#test`. Heavy; the
          flake check is the cheap gate.

Task 7: REWRITE packaging/nix/README.md (verbatim from the "What" section)
  - WRITE: the full README above. New "NixOS (recommended)" section (the module
          usage with the flake.nix + configuration.nix snippets); the old manual
          section relabeled "Non-NixOS (Nix on another distro)" with the
          command -v qmkonnect-hid-id path-rewrite; the "future update" sentence
          replaced.

Task 8: FINAL CHECKS
  - RUN: git status --short -> ONLY flake.nix + packaging/nix/README.md changed.
  - RUN: git diff -- Cargo.toml packaging/linux/ src/ docs/ -> empty (no source change).
  - CONFIRM: the udev-rule + service-template SOURCE files are byte-identical
          (the rewrite is build-time only).
```

### Implementation Patterns & Key Details

```nix
# === WHY rewrite FHS paths at BUILD time (not edit the sources) ===
#   The udev rule + service template are SHARED across all Linux channels (Arch
#   PKGBUILD, AUR, the generic install). They're correct FHS-pathed for those. Only
#   NixOS lacks the FHS, so only the Nix BUILT copy is rewritten (postInstall).
#   Editing the sources would break Arch/Ubuntu. substituteInPlace/substitute at
#   build time is the canonical Nix way to FHS-ify a package's shipped config.

# === WHY copy (not symlink) the helper to $out/lib/udev/ ===
#   Parity with the Arch PKGBUILD (install -Dm755) + avoids any udev IMPORT
#   symlink-resolution edge case in the early-boot udev context. The helper is a
#   tiny pure-std binary; the store duplication is negligible (Nix dedups).

# === WHY nixosModules is OUTSIDE eachSystem ===
#   A NixOS module is a single function applied to whatever host imports it — it's
#   system-agnostic. eachSystem makes per-system attrsets (packages.x86_64-linux…).
#   So the module is a SIBLING: `let perSystem = (eachSystem …); in perSystem // {
#   nixosModules.default = …; }`. The `//` merges the system-agnostic attr beside
#   the per-system ones.

# === HOW the module references its own package ===
#   self.packages.${pkgs.system}.qmkonnect. `self` is captured by the outputs
#   closure; `pkgs.system` is the host system inside the module. This resolves to
#   the same package (with postInstall) that nix build produces. No cycle.

# === HOW "enable the user service" works on base NixOS ===
#   systemd.packages = [pkg] links the package's lib/systemd/user/qmkonnect.service
#   into /etc/systemd/user/ (the system-wide user-unit library). The udev rule's
#   ENV{SYSTEMD_USER_WANTS}+=qmkonnect.service then auto-starts it on device plug.
#   For login-time start, the user runs `systemctl --user enable --now qmkonnect`
#   once. Base NixOS has no declarative systemd.user.services (home-manager does),
#   so this is the correct ceiling.

# === WHY services.qmkonnect.user is OPTIONAL ===
#   The udev rule sets GROUP="input" (fallback) + TAG+="uaccess" (primary). On a
#   normal NixOS desktop with logind, uaccess grants the active user access via
#   session ACL — no group membership needed. input is only the replug-race
#   fallback. So the user option defaults to null; setting it augments that user's
#   extraGroups (the user must be defined elsewhere in the config).
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "flake.nix (postInstall + nixosModules + outputs restructure),
             packaging/nix/README.md (NixOS section + relabeled manual)"
  - do NOT modify: "Cargo.toml, src/, packaging/linux/udev/*.rules (SOURCE),
                    packaging/linux/systemd/*.template (SOURCE), docs/,
                    packaging/linux/arch/*"

PUBLIC API SURFACE:
  - flake now exposes (in addition to S1's outputs):
    nixosModules.default — a NixOS module importable via
    inputs.qmkonnect.nixosModules.default.
  - packages.${system}.qmkonnect now ships 5 files (2 bins + helper + rule + service).

DEPENDENCIES / Cargo.toml:
  - none new. The postInstall uses stdenv setup hooks (install, substitute,
    substituteInPlace) — always available in buildRustPackage. No new Nixpkgs input.

UPSTREAM/DOWNSTREAM:
  - UPSTREAM: S1 (flake.nix + README — on disk); the udev/systemd SOURCE files;
    nixpkgs (systemd/udev module options).
  - DOWNSTREAM: P1.M5.T2.S2 adds `nix flake check` to CI (validates the
    nixosModules.default output this task ships).

CI:
  - external_deps.md §5: "Flake lives in-repo; validation in CI (nix flake check)".
    CI integration is P1.M5.T2.S2 — NOT this task (this task ships the module).

OUT OF SCOPE:
  - home-manager integration (a homeModules output) — base NixOS module only.
  - A declarative systemd.user.services option — doesn't exist in base NixOS.
  - macOS/Windows flake outputs — native installers exist; Nix is Linux-only here.
  - Editing the udev/systemd SOURCE files — build-time rewrite only.
```

## Validation Loop

> **⚠ ENV GATE.** `nix` is NOT installed in the authoring env. The implementer runs
> every command below in a **Nix-capable env** (NixOS, or Nix on any Linux). The
> inherited cargoHash + hidapi-link iterations (from S1) are PRECONDITIONS: the
> postInstall only runs once `nix build` succeeds.

### Level 1: Flake evaluates (Nix env)

```bash
cd /home/dustin/projects/qmkonnect

# Static eval: catches Nix-expr errors in the module + the restructured outputs
# WITHOUT a full build.
nix flake show 2>&1 | head
# Expected: lists packages.{x86_64,aarch64}-linux.{qmkonnect,default},
#   devShells.{x86_64,aarch64}-linux.default. (nixosModules is NOT shown by
#   `nix flake show` by default, but `nix flake check` evals it — see Level 3.)

nix eval .#nixosModules.default --apply "m: builtins.functionArgs m"
# Expected: { config = true; lib = true; pkgs = true; options = true; ... }
#   (confirms nixosModules.default is a module function that evals.)
```

### Level 2: Build + verify the postInstall output (Nix env)

```bash
cd /home/dustin/projects/qmkonnect

# Precondition: the inherited cargoHash iteration (fakeHash -> real). If
# cargoHash is still pkgs.lib.fakeHash, the first build FAILS with "got:
# sha256-<REAL>" — paste it, rebuild. (Task 3.)
nix build .#qmkonnect

# (Task 4, only if needed): if the build fails "cannot find -lhidapi-hidraw",
# set RUSTFLAGS = "" in flake.nix, rebuild.

# Verify the 5 files shipped (2 bins + helper + rule + service).
find result -type f \( -name 'qmkonnect' -o -name 'qmkonnect-hid-id' -o -name '*.rules' -o -name 'qmkonnect.service' \)
# Expected: result/bin/qmkonnect, result/bin/qmkonnect-hid-id,
#   result/lib/udev/qmkonnect-hid-id, result/lib/udev/rules.d/69-qmkonnect-rawhid.rules,
#   result/lib/systemd/user/qmkonnect.service.

# Verify the FHS paths were rewritten to the Nix store path.
grep -c '/usr/lib/udev/qmkonnect-hid-id' result/lib/udev/rules.d/69-qmkonnect-rawhid.rules
# Expected: 0  (rewritten). A non-zero means substituteInPlace didn't fire — fix it.
grep -o '/nix/store/[a-z0-9]*-qmkonnect-[0-9.]*/lib/udev/qmkonnect-hid-id' result/lib/udev/rules.d/*.rules
# Expected: the store path (proves the rewrite landed).
grep '^ExecStart=' result/lib/systemd/user/qmkonnect.service
# Expected: ExecStart=/nix/store/<hash>-qmkonnect-<ver>/bin/qmkonnect  (NOT /usr/bin/qmkonnect).
```

### Level 3: Flake check + module eval (Nix env)

```bash
cd /home/dustin/projects/qmkonnect

# nix flake check evals ALL outputs incl. nixosModules.default. A Nix-expr error
# in the module (typo, missing lib.mdDoc, bad mkIf) fails here.
nix flake check
# Expected: passes (exit 0).

# (Optional, deeper) Prove the module integrates into a NixOS config without a
# circular ref or eval error. Add a throwaway nixosConfigurations.test to a
# separate flake (or a temp file), import nixosModules.default + enable, build a VM:
cat >/tmp/qmk-test.nix <<'EOF'
{ inputs, ... }: {
  nixosConfigurations.test = inputs.nixpkgs.lib.nixosSystem {
    system = "x86_64-linux";
    modules = [ (inputs.qmkonnect.nixosModules.default) ./test-host.nix ];
  };
}
EOF
# (test-host.nix: { services.qmkonnect.enable = true; } )
nix run nixpkgs#nixos-rebuild -- build-vm --flake /tmp/qmk-test.nix#test
# Expected: builds a VM image (heavy; confirms udev + systemd + the package all
#   eval + integrate). The `nix flake check` above is the cheap gate — this is
#   belt-and-suspenders.
```

### Level 4: Docs + scope hygiene (any env)

```bash
cd /home/dustin/projects/qmkonnect

# The README has the NixOS section + the relabeled non-NixOS manual section.
grep -q 'NixOS (recommended)' packaging/nix/README.md       # the new section
grep -q 'nixosModules.default' packaging/nix/README.md       # the module import
grep -q 'services.qmkonnect.enable = true' packaging/nix/README.md
grep -q 'Non-NixOS (Nix on another distro)' packaging/nix/README.md   # relabeled manual
! grep -q 'A future update will ship' packaging/nix/README.md          # old note gone

# Scope hygiene: ONLY flake.nix + packaging/nix/README.md changed.
git status --short
# Expected: ONLY flake.nix + packaging/nix/README.md (both modified).
git diff --stat -- Cargo.toml packaging/linux/ src/ docs/
# Expected: empty (no source/udev-source/systemd-source/docs change).

# The udev-rule + service-template SOURCE files are byte-identical (build-time rewrite only).
git diff --stat -- packaging/linux/udev/ packaging/linux/systemd/
# Expected: empty.
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1: `nix eval .#nixosModules.default …` confirms the module is a function.
- [ ] Level 2: `nix build .#qmkonnect` succeeds (after the cargoHash iteration + the
      optional hidapi fallback).
- [ ] Level 2: the 5 files exist in `result/` (2 bins + helper + rule + service).
- [ ] Level 2: the rule's helper path + the service's ExecStart are the Nix store
      path (0 `/usr/lib/udev/qmkonnect-hid-id` / `/usr/bin/qmkonnect` in the built
      copies).
- [ ] Level 3: `nix flake check` passes (evals `nixosModules.default`).
- [ ] Level 4: the SOURCE udev rule + service template are byte-identical
      (build-time rewrite only).

### Feature Validation

- [ ] `packages.qmkonnect` has a `postInstall` (3 steps: helper, rule+rewrite,
      service+instantiate+rewrite).
- [ ] `outputs` is `let perSystem = (eachSystem …); in perSystem // { nixosModules.default = …; }`.
- [ ] `nixosModules.default` has `options.services.qmkonnect.{enable, user}` and
      `config = mkIf cfg.enable { environment.systemPackages; services.udev.packages;
      systemd.packages; optional input-group }`.
- [ ] `packaging/nix/README.md` has the "NixOS (recommended)" section + relabeled
      "Non-NixOS" manual; the "future update" note is gone.
- [ ] `BindsTo=dev-qmkonnect_device.device` is UNCHANGED (FHS-independent).

### Code Quality Validation

- [ ] `flake.nix` restructure is idiomatic (`let perSystem … in perSystem // {…}`).
- [ ] The module uses `mkEnableOption` + `mkIf` (the universal NixOS module convention).
- [ ] The postInstall uses stdenv hooks (`install -D`, `substituteInPlace`, `substitute`).
- [ ] Only 2 files modified; no source/udev-source/systemd-source/docs change.

### Documentation & Deployment

- [ ] Mode-A README rides with the work (NixOS section + non-NixOS manual).
- [ ] The README cross-refs `docs/installation.md` + `spec/LINUX.md §6` (no
      duplication of the udev/systemd command details).
- [ ] No Cargo.toml / CI / src/ change.

---

## Anti-Patterns to Avoid

- ❌ Don't edit the udev-rule / service-template SOURCE files to remove the FHS
  paths — they're SHARED with Arch/Ubuntu/AUR (correct FHS-pathed there). The
  rewrite happens at BUILD time in `postInstall` (NixOS-only). Editing sources
  breaks the other channels.
- ❌ Don't rewrite `BindsTo=dev-qmkonnect_device.device` — it names the device unit
  from the `qmkonnect_device` SYMLINK (FHS-independent); it works on NixOS as-is.
  "Fixing" it breaks the unplug-stop / boot-wait binding.
- ❌ Don't put `nixosModules` INSIDE `eachSystem` — a NixOS module is
  system-agnostic (one function applied to any host). It must be a SIBLING:
  `let perSystem = (eachSystem …); in perSystem // { nixosModules.default = …; }`.
- ❌ Don't add a `systemd.user.services.qmkonnect` option in the module — base NixOS
  has no such option (it's home-manager). "Enable" the user service via
  `systemd.packages` (makes it available in /etc/systemd/user/) + the udev rule's
  `SYSTEMD_USER_WANTS` (auto-start on plug).
- ❌ Don't force the `input` group on every user — `uaccess` (in the rule) is the
  PRIMARY permission on a logind desktop; `input` is the replug-race fallback. Make
  `services.qmkonnect.user` OPTIONAL (default null); only set it if you rely on the
  raw group permission.
- ❌ Don't reference the package via `inputs.self.packages.${system}` inside the
  module — `self` is captured by the outputs closure directly; use
  `self.packages.${pkgs.system}.qmkonnect`. (And `pkgs.system`, not a hardcoded
  system string, so the module is portable.)
- ❌ Don't use a surgical `edit` to wrap `outputs = … flake-utils.lib.eachSystem …`
  into `let perSystem = … in … // …` — the wrapping spans the whole outputs body
  and is error-prone. Rewrite `flake.nix` wholesale (the diff is reviewable; every
  S1 byte is preserved).
- ❌ Don't forget the inherited `cargoHash` iteration — `postInstall` only runs once
  `nix build .#qmkonnect` succeeds, which requires the real cargoHash (the fakeHash
  on disk fails the cargo-fetch phase). The two iterations are independent.
- ❌ Don't symlink the helper to `$out/lib/udev/` "to save space" — copy it
  (`install -Dm755`) for parity with the Arch PKGBUILD and to avoid any udev IMPORT
  symlink-resolution edge case in early-boot udev. (Nix dedups store contents; the
  duplication is negligible.)
- ❌ Don't claim validation "passes" without a Nix env — `nix` is not in the
  authoring env. The PRP is honest: the gate is env-gated; the cargoHash/hidapi
  iterations are standard Nix ceremony, not failures.
- ❌ Don't add `--no-default-features` or change the buildInputs/RUSTFLAGS — those
  are S1's (build-parity with the Arch PKGBUILD). This task only ADDS the
  postInstall + the module; it doesn't touch the build recipe.
- ❌ Don't duplicate the udev/systemd command details in the README — cross-ref
  `docs/installation.md` + `spec/LINUX.md §6` (S1's discipline; S2 keeps it).

---

**Confidence Score: 8/10** for one-pass implementation success. The deliverable is
two verbatim files (the full `flake.nix` with a `postInstall` using the canonical
`install`/`substitute`/`substituteInPlace` hooks + a `let perSystem … in perSystem //
{ nixosModules.default }` restructure + a standard `mkEnableOption`/`mkIf` NixOS
module, and the full README with the NixOS section). The two NixOS-correctness
gotchas (the rule's `/usr/lib/udev/qmkonnect-hid-id` IMPORT path + the service's
`/usr/bin/qmkonnect` ExecStart must be rewritten to store paths; `BindsTo` must
NOT) are spelled out with the exact `--replace` strings. The NixOS module's three
mechanisms (`services.udev.packages`, `systemd.packages`, the optional
`services.qmkonnect.user` → `input` group) each have a documented rationale tied to
the NixOS wiki + the PRD's uaccess/GROUP note. The 2-point deduction reflects the
**env gate** (`nix` not in the authoring env) plus two things that can only be
confirmed in a Nix env: (1) the inherited `cargoHash` iteration must complete before
`postInstall` runs, and (2) `systemd.packages` reliably links the `lib/systemd/user/`
unit into `/etc/systemd/user/` on the implementer's Nixpkgs revision (if not, the
documented fallback is `environment.etc."systemd/user/qmkonnect.service"`). Both are
standard Nix workflows with clear symptoms; the runbook keys the resolution to the
exact output. The scope boundary vs S1 (binaries) + the AUR sibling is explicit, so
no work is duplicated or conflicting. Given a Nix env, a competent implementer lands
this in one pass; the cargoHash ceremony is inherited, not introduced here.