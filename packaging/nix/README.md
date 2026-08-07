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