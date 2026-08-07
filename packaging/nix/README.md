# QMKonnect — Nix flake

QMKonnect ships a [Nix flake](https://nixos.wiki/wiki/Flakes) that builds the app
**from source** against pinned [Nixpkgs](https://github.com/NixOS/nixpkgs), with
all Linux system dependencies (GTK3, hidapi, libxdo, …) provided automatically.

## Install

```sh
# Add to your user profile (recommended):
nix profile install github:dabstractor/qmkonnect

# …or run ad-hoc without installing:
nix run github:dabstractor/qmkonnect

# …or build into ./result without installing:
nix build github:dabstractor/qmkonnect
# binary: ./result/bin/qmkonnect   (helper: ./result/bin/qmkonnect-hid-id)
```

The flake builds both binaries — `qmkonnect` (the app) and `qmkonnect-hid-id`
(the udev helper) — for `x86_64-linux` and `aarch64-linux`.

## Post-install: udev rule + systemd service (one-time, manual)

**Nix is per-user and cannot install udev rules system-wide**, so the HID
permissions + optional autostart service are a one-time manual step (identical to
the generic Linux install). From a checkout of the repo (or the installed
`qmkonnect-hid-id`):

```sh
# 1. Static udev rule — IDENTICAL for every keyboard (no per-VID/PID config).
sudo install -m644 packaging/linux/udev/69-qmkonnect-rawhid.rules \
  /usr/lib/udev/rules.d/

# 2. The udev helper the rule IMPORTs (adjust the path in the rule if you place
#    it elsewhere). It tags any hidraw interface carrying the QMK Raw HID
#    signature (usage page 0xFF60 / usage 0x61).
sudo install -m755 result/bin/qmkonnect-hid-id /usr/lib/udev/qmkonnect-hid-id

# 3. Reload udev so the rule + permissions take effect.
sudo udevadm control --reload-rules && sudo udevadm trigger
```

For the optional systemd user service (autostart at login), see the **Linux
install** section of [docs/installation.md](../../docs/installation.md) and
`spec/LINUX.md §6` — instantiate
`packaging/linux/systemd/qmkonnect.service.template` the same way as a non-Nix
install. (A future update will ship the rule + helper + template from the Nix
package itself.)

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