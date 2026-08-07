# QMKonnect — Debian / Ubuntu / Linux Mint package (`.deb`)

This directory holds the **native Debian package** for QMKonnect, built with
[`cargo-deb`](https://github.com/kornelski/cargo-deb) from the
`[package.metadata.deb]` block in [`Cargo.toml`](../../Cargo.toml)
([`spec/PACKAGING.md`](../../spec/PACKAGING.md) §4.3).

It is the **`.deb` sibling** of the [Arch source/binary packages](../linux/aur/README.md)
and the [Nix flake](../nix/README.md): it installs the **same six artifacts**
to the **same FHS paths** ([`spec/PACKAGING.md`](../../spec/PACKAGING.md) §4)
and reuses the **same install/uninstall lifecycle** the Arch
`qmkonnect.install` proves out — just wrapped in dpkg metadata and translated to
POSIX-`sh` maintainer scripts (`postinst` / `prerm` / `postrm`).

## What this is

`cargo-deb` reads the `[package.metadata.deb]` table in `Cargo.toml`, packages
the release binaries + the four data assets (udev helper, static udev rule,
systemd user-service template, XDG autostart `.desktop`, README) into a `.deb`,
and wires the three maintainer scripts into the control archive. The result is a
one-liner `sudo apt install ./…` install with proper `Depends` resolution.

## Build

Build **on Debian/Ubuntu/Mint** (the unified hidapi auto-selects the hidraw
backend — **no** `-lhidapi-hidraw` link flag; see
[`spec/PACKAGING.md`](../../spec/PACKAGING.md) §2):

```bash
cargo install cargo-deb          # one-time: install the cargo-deb subcommand
cargo build --release            # produce target/release/{qmkonnect,qmkonnect-hid-id}
cargo deb                        # produce target/debian/qmkonnect_0.2.8_amd64.deb
```

Output: `target/debian/qmkonnect_0.2.8_amd64.deb`

> **CI is the authoritative build.** The release pipeline builds on
> `ubuntu-22.04` (glibc 2.35) so the runtime works on 22.04, 24.04, Debian 12,
> and Mint 21/22+, then renames the artifact to
> `qmkonnect-0.2.8-linux-amd64.deb` for the GitHub Release (CI wiring is
> P1.M7.T1.S2).

### Build dependencies (apt)

```bash
sudo apt install libhidapi-dev libxdo-dev pkg-config
```

## Install

```bash
# Option A — dpkg (does NOT auto-resolve Depends; install them yourself):
sudo dpkg -i target/debian/qmkonnect_*.deb

# Option B — apt (auto-resolves the Depends from the local file):
sudo apt install ./target/debian/qmkonnect_*.deb
```

### Runtime dependencies (`Depends`)

The `.deb` declares and apt resolves these automatically (Option B):

- `libhidapi-hidraw0` — the hidraw backend the unified `libhidapi.so` dlopens
- `libxdo3` — X11 window-class lookup
- `zenity` — the Settings dialog (`spec/UI.md`)
- `libnotify-bin` — `notify-send` for "Window Information" + status toasts
- `systemd` — the user service the postinst instantiates + globally enables

## What it installs

| Path | What |
|---|---|
| `/usr/bin/qmkonnect` | The daemon binary |
| `/usr/lib/udev/qmkonnect-hid-id` | udev helper tagging QMK Raw HID interfaces |
| `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` | Static usage-page udev rule |
| `/usr/lib/systemd/user/qmkonnect.service.template` | systemd user service template (instantiated by `postinst`) |
| `/etc/xdg/autostart/qmkonnect.desktop` | XDG autostart entry — starts the daemon at login on systemd **and** non-systemd distros |
| `/usr/share/doc/qmkonnect/README.md` | The project README |
| `/usr/share/doc/qmkonnect/copyright` | `LICENSE` (MIT), shipped verbatim via `license-file = ["LICENSE","0"]`) |

These paths are the canonical FHS layout — see
[`spec/PACKAGING.md`](../../spec/PACKAGING.md) §4.

## Install / uninstall hooks

The maintainer scripts in this directory (`postinst`, `prerm`, `postrm`) are
POSIX-`sh` translations of [`packaging/linux/arch/qmkonnect.install`](../linux/arch/qmkonnect.install)
(no bashisms — Debian runs them under `/bin/sh` = dash):

- **`postinst`** (install/upgrade): instantiate
  `qmkonnect.service` from the template → reload + trigger udev → ensure the
  `input` group exists (`getent … || addgroup`, idempotent) →
  `systemctl --global enable qmkonnect.service` → print zero-config next-steps.
- **`prerm`**: documented no-op (the running per-user service is left alone
  until reboot or an explicit `systemctl --user stop`).
- **`postrm`** (remove/purge): `systemctl --global disable` → stop + disable
  per-user instances → remove the instantiated `.service` and any user-generated
  `/etc/udev/rules.d/99-qmkonnect.rules` → reload + trigger udev.

Default QMK keyboards then need **no configuration**: QMKonnect auto-discovers
them by the standard Raw HID usage page (`0xFF60` / `0x61`), and the shipped
static udev rule already grants permissions. See the **Linux install** section of
[`docs/installation.md`](../../docs/installation.md) for the optional
per-board config + on-demand udev-rule workflow.

## hidapi link note

Unlike the Arch PKGBUILD (which passes
`RUSTFLAGS="-C link-arg=-lhidapi-hidraw"` because Arch ships the hidraw/libusb
backends as **separate** libraries), the `.deb` build uses **no** hidraw flag:
Debian/Ubuntu/Mint ship a **unified** hidapi (≥0.14) in `libhidapi.so` that
auto-selects the hidraw backend at runtime, so usage/usage_page device matching
works without it. Adding the flag here would mis-link the backend and break
matching. See [`spec/PACKAGING.md`](../../spec/PACKAGING.md) §2.