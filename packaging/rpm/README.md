# QMKonnect — Fedora / RHEL / Rocky / Alma / openSUSE package (`.rpm`)

This directory holds the **native RPM package** for QMKonnect, built with
[`cargo-generate-rpm`](https://github.com/cat-in-136/cargo-generate-rpm) from the
`[package.metadata.generate-rpm]` block in [`Cargo.toml`](../../../Cargo.toml)
([`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §4.4).

It is the **`.rpm` sibling** of the [Debian package](../debian/README.md), the
[Arch source/binary packages](../linux/aur/README.md), and the
[Nix flake](../nix/README.md): it installs the **same artifact set** to the
**same FHS paths** ([`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §4) and
reuses the **same install/uninstall lifecycle** the Arch `qmkonnect.install`
proves out — just wrapped in RPM metadata and translated to POSIX-`sh`
maintainer scriptlets (`postin` = `%post`, `postun` = `%postun`).

## What this is

`cargo-generate-rpm` reads the `[package.metadata.generate-rpm]` table in
`Cargo.toml`, packages the release binaries + the data assets (udev helper,
static udev rule, systemd user-service template, XDG autostart `.desktop`,
README, LICENSE) into a `.rpm`, declares the Fedora runtime requires, and embeds
the two maintainer scriptlets. The result is a one-liner
`sudo dnf install …` install with proper `Requires` resolution.

> **Spec correction.** `spec/PACKAGING.md` §4.4 prints a
> `require-local = { "hidapi" >= "0.10", … }` line that is **invalid TOML** and a
> **nonexistent** cargo-generate-rpm field. The `[package.metadata.generate-rpm]`
> block in `Cargo.toml` instead uses the upstream-correct
> `[package.metadata.generate-rpm.requires]` sub-table for versioned runtime
> requires (`hidapi = ">= 0.10"`, with a mandatory space after `>=`). The spec is
> read-only; this correction lives at the implementation layer.

## Build

Build **on Fedora/RHEL** (the unified hidapi auto-selects the hidraw backend —
**no** `-lhidapi-hidraw` link flag; see [`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §2):

```bash
cargo install cargo-generate-rpm   # one-time: install the cargo-generate-rpm subcommand
cargo build --release              # produce target/release/{qmkonnect,qmkonnect-hid-id}
cargo generate-rpm                 # produce target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm
```

Output: `target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm`

> **`cargo generate-rpm` does NOT build for you** — run `cargo build --release`
> first. Our `[profile.release] strip = true` already strips the binaries (no
> manual `strip -s` needed).

> **CI is the authoritative build.** The release pipeline builds on Fedora
> (glibc + unified hidapi) and renames the artifact to
> `qmkonnect-<ver>-linux-x86_64.rpm` for the GitHub Release (CI wiring is
> P1.M7.T2.S2).

### Build dependencies (dnf)

```bash
sudo dnf install hidapi-devel libxdo-devel pkgconf-pkg-config \
  gtk3-devel glib2-devel libappindicator-gtk3-devel \
  libX11-devel libxcb-devel systemd-devel
```

> **Arch-host local-validation note.** This dev box is Arch, which ships the
> hidraw/libusb backends as **separate** libraries (unlike Fedora's unified
> hidapi). For a LOCAL structural validation build only, set
> `RUSTFLAGS="-C link-arg=-lhidapi-hidraw"` for the `cargo build --release` step,
> then run `cargo generate-rpm`. The resulting `.rpm` is structurally valid
> (correct files/requires/scriptlets) but linked against Arch glibc — **not for
> release**. The flag is never baked into the recipe; the authoritative build is
> Fedora CI.

## Install

```bash
# Option A — dnf (auto-resolves Requires from the local file):
sudo dnf install target/generate-rpm/qmkonnect-*.rpm

# Option B — rpm (does NOT auto-resolve; install deps yourself first):
sudo rpm -i target/generate-rpm/qmkonnect-*.rpm
```

### Runtime dependencies (`Requires`)

The `.rpm` declares these explicitly in
`[package.metadata.generate-rpm.requires]`, and `dnf install` resolves them
automatically (Option A):

- `hidapi >= 0.10` — the unified hidapi that auto-selects the hidraw backend
- `libxdo` — X11 window-class lookup
- `zenity` — the Settings dialog ([`spec/UI.md`](../../../spec/UI.md))
- `libnotify` — `notify-send` for "Window Information" + status toasts
- `systemd` — the user service the postin instantiates + globally enables

`cargo-generate-rpm`'s auto-require detection (on by default) additionally adds
the library-level `libfoo.so.N()` requires (`libhidapi.so.0()(64bit)`, etc.),
which coexist with the explicit package-level requires above.

> **openSUSE** shares this spec with package names `HIDAPI`, `libxdo-devel`,
> `libnotify-tools`, `zenity` ([`spec/PACKAGING.md`](../../../spec/PACKAGING.md)
> §4.4); an OBS (openSUSE Build Service) submit is a community follow-on.

## What it installs

| Path | What |
|---|---|
| `/usr/bin/qmkonnect` | The daemon binary |
| `/usr/lib/udev/qmkonnect-hid-id` | udev helper tagging QMK Raw HID interfaces |
| `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` | Static usage-page udev rule |
| `/usr/lib/systemd/user/qmkonnect.service.template` | systemd user service template (instantiated by `postin`) |
| `/etc/xdg/autostart/qmkonnect.desktop` | XDG autostart entry — starts the daemon at login on systemd **and** non-systemd distros |
| `/usr/share/doc/qmkonnect/README.md` | The project README |
| `/usr/share/licenses/qmkonnect/LICENSE` | `LICENSE` (MIT) — shipped as an explicit asset (cargo-generate-rpm has no `license-file` mechanism) |

These paths are the canonical FHS layout — see
[`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §4.

## Install / uninstall hooks

The scriptlets in this directory (`postin`, `postun`) are POSIX-`sh`
translations of [`packaging/linux/arch/qmkonnect.install`](../linux/arch/qmkonnect.install)
(no bashisms — RPM runs them under `/bin/sh`):

- **`postin`** (`%post`, install/upgrade): instantiate
  `qmkonnect.service` from the template → reload + trigger udev → ensure the
  `input` group exists (`getent … || groupadd -r`, idempotent — `groupadd -r` is
  the Fedora/RHEL system-group syntax, not Debian's `addgroup`) →
  `systemctl --global enable qmkonnect.service` → print zero-config next-steps.
  No `$1` guard: the logic is idempotent and desirable on upgrade.
- **`postun`** (`%postun`, erase/upgrade): cleanup is **erase-guarded** with
  `if [ "$1" = "0" ]` so it only runs on complete removal. On `dnf upgrade`,
  `%postun` fires for the **old** package right after the **new** one lands
  (`$1 = 2`); the guard ensures the upgrade does NOT tear down the service +
  rules the new package just installed. When it does run (erase): global disable
  → stop + disable per-user instances → remove the instantiated `.service` and
  any user-generated `/etc/udev/rules.d/99-qmkonnect.rules` → reload + trigger
  udev.

Default QMK keyboards then need **no configuration**: QMKonnect auto-discovers
them by the standard Raw HID usage page (`0xFF60` / `0x61`), and the shipped
static udev rule already grants permissions. See the **Linux install** section of
[`docs/installation.md`](../../../docs/installation.md) for the optional
per-board config + on-demand udev-rule workflow.

## hidapi link note

Unlike the [Arch PKGBUILD](../linux/arch/PKGBUILD) (which passes
`RUSTFLAGS="-C link-arg=-lhidapi-hidraw"` because Arch ships the hidraw/libusb
backends as **separate** libraries), the `.rpm` build uses **no** hidraw flag:
Fedora/RHEL/Rocky/Alma/openSUSE ship a **unified** hidapi (≥0.14) in
`libhidapi.so` that auto-selects the hidraw backend at runtime, so
usage/usage_page device matching works without it. Adding the flag here would
mis-link the backend and break matching. See
[`spec/PACKAGING.md`](../../../spec/PACKAGING.md) §2.