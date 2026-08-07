# QMKonnect — Validation Report

**Date:** 2026-08-07 · **Version validated:** 0.2.8 (`Cargo.toml`)
**Validation script:** `./validate.sh` — **65 checks passed · 1 failed · 2 warnings · 0 skipped** (live run)
**Host:** Arch Linux x86_64 (kernel 7.1), Rust 1.92.0, Hyprland (Wayland), with a real qmk_notifier-capable keyboard attached.

---

## TL;DR

QMKonnect is a **healthy, production-grade** desktop daemon. F1–F17 are
implemented and the **entire end-to-end pipeline was verified live against real
hardware**: runtime backend selection → Tier-2 `QUERY_INFO` capability handshake
(proto v2 capable) → window detection → debounced Raw-HID string send + host-
context typed send. All **441** unit tests pass single-threaded; **clippy is
clean** (`-D warnings`); the **`.deb` and `.rpm` both build** with every asset +
maintainer script at the correct FHS paths.

Validation surfaced **1 hard issue** and **2 feature-claim gaps**. None affect
the running app's correctness for an existing user — they are about CI hygiene
and the accuracy of two advertised distribution channels.

| Severity | Count | Items |
|---|---|---|
| 🔴 Hard (blocks the `main`-push CI gate) | 1 | `cargo fmt --check` regression |
| 🟡 Feature gap (advertised channel non-functional) | 1 | Nix flake uses `fakeHash` → `nix run`/`build` fails |
| 🟠 Doc/UX drift | 1 | `qmkonnect -l` understates the backends the build ships |

> **Correction vs. the prior report in this file:** the previous report claimed
> the `.deb`/`.rpm` were "missing/broken" and clippy failed. That is **not the
> current state** — both packages build cleanly (`cargo deb` → 960 KB, `cargo
> generate-rpm` → 1.1 MB) with all 6–7 assets and maintainer scripts, and
> `cargo clippy --all-targets -- -D warnings` exits 0. The stale tasks.json
> statuses (`Ready`/`Planned`) lag the implemented code.

---

## 🔴 Hard Issue

### H1. `cargo fmt --all -- --check` fails — committed code is unformatted

**Impact:** `.github/workflows/ci.yml` runs `cargo fmt --all -- --check` in the
`fmt` job on **every push to `main`**. The repository currently fails this gate,
so the next push to `main` will turn CI red.

**Evidence:**
```
$ cargo fmt --all -- --check ; echo $?
1
```
- **10** formatting hunks across **4 files**:
  - `src/core/mod.rs`
  - `src/platforms/atspi.rs`
  - `src/platforms/mod.rs`
  - `src/platforms/wayland_ft.rs`

The diffs are cosmetic (whitespace alignment / line collapsing), e.g.
```diff
-                _ => Some(b),         // force the named backend
+                _ => Some(b),        // force the named backend
```

**Fix (trivial):** `cargo fmt --all` and commit. No behaviour change.

---

## 🟡 Feature Gap

### M1. The Nix flake cannot build — `nix run`/`nix build`/`nix profile install` all fail

**Impact:** PRD §4 F15 lists "a **Nix** flake" as a shipped distribution channel;
`README.md` advertises `nix run github:dabstractor/qmkonnect`; the Package
Managers table says `nix profile install`. **None of these work today** because
`flake.nix` ships `cargoHash = pkgs.lib.fakeHash;` (a deliberate placeholder).

A user running any Nix command gets a fixed-output hash mismatch and the build
aborts. The CI jobs (`ci.yml` `nix-check` and `release.yml` `nix`) deliberately
pass `nix flake check --no-build` to **avoid** this — i.e. CI only verifies the
flake *evaluates*, never that it *builds*. Both workflow files document this as
an out-of-scope follow-up, but it leaves the advertised user-facing channel
broken.

**Evidence:**
```
flake.nix:52:            cargoHash = pkgs.lib.fakeHash;
```
and (from both workflows):
```
# WHY --no-build (load-bearing): flake.nix ships with cargoHash = fakeHash …
# A BUILDING `nix flake check` would FAIL with a hash mismatch until …
```

**Fix:** one-time iteration — run `nix build .#qmkonnect`, read the
`got: sha256-…` from the failure, paste it into `flake.nix` in place of
`fakeHash`, rebuild to confirm, then drop `--no-build` from both CI jobs (and add
`checks.*` if desired). The flake structure, NixOS module, and postInstall are
already correct — only the hash is missing.

---

## 🟠 Doc / UX Drift

### M2. `qmkonnect -l` understates the backends this build ships (F16)

**Impact:** Minor user-facing inaccuracy. `CONFIG.md` §4 documents `-l`/`--list`
as "List supported platforms (this build)". Today it prints only:

```
Supported platforms (this build):
  Linux (Hyprland)
```

…even though the build's `default = ["wayland","gnome","atspi","hyprland", …]`
feature set ships the **foreign-toplevel Wayland** backend (which is what
actually gets selected at runtime — the verbose log shows
`select_linux_backend: … 'foreign-toplevel' available, selected`), plus GNOME
and AT-SPI backends and unconditional X11 (`PLATFORMS.md` §6).

The `print_platforms()` function in `src/main.rs` still keys off the legacy
`cfg(feature = "hyprland")` either/or, predating F16's runtime multi-backend
selection. The verbose startup path is correct; only the `-l` summary is stale.

**Fix:** update `print_platforms()` to list the compiled-in backends (or at
least say "Linux (multi-backend: foreign-toplevel / GNOME / Hyprland / AT-SPI /
X11 — runtime-selected)"). Cosmetic; no behaviour change.

---

## What Was Verified Healthy (the other 65 checks)

### Toolchain & quality gates
- ✅ `cargo build --release --all-targets` — clean.
- ✅ `cargo clippy --all-targets -- -D warnings` — **exit 0** (the prior report's
  "clippy regression" is resolved).
- ✅ `cargo test --bin qmkonnect -- --test-threads=1` — **441 passed, 0 failed**
  (single-threaded as mandated by AGENTS.md / shared debouncer state).
- ✅ `cargo test --bin qmkonnect-hid-id` — udev-helper parser tests pass.

### Wire-protocol contract (PROTOCOL.md §7) — deep check against the pinned crate
The framing lives in the git-tagged `qmk-notifier` v0.3.0 (Cargo.lock sha
`f26893e`). Verified every constant in `src/core.rs`:
- ✅ `DEFAULT_USAGE_PAGE = 0xFF60`, `DEFAULT_USAGE = 0x61`, `REPORT_LENGTH = 32`.
- ✅ Magic header `request_data[1]=0x81`, `[2]=0x9F` on every 33-byte report.
- ✅ `ETX_TERMINATOR_BYTE = 0x03` (appended by the crate, not the app).
- ✅ Typed-command discriminator `CMD_DISCRIMINATOR = 0xF0`, response marker `0x51`.
- ✅ `PAYLOAD_PER_REPORT = REPORT_LENGTH - 2 = 30`; `IN_DRAIN_MAX = 32`.
- ✅ App payload build is exactly `format!("{class}\x1D{title}")` (GS = 0x1D).
- ✅ `r_coex_invariants` test module asserts every transport variant emits `0x81`
  as its first on-wire byte (the VIA-coexistence / protocol-demux guarantee,
  DEVICE_DISCOVERY.md §6.4).

### Host-side rules (HOST_RULES.md) — stack/replace/no-match dispatch (C13)
- ✅ `dispatch_window_send` matches the spec: `None` → string only; non-replace
  (stack **or** no-match) → string first **then** `ApplyHostContext{clear_board:false}`
  (board untouched on no-match); replace → context only, `clear_board:true`.
- ✅ `--validate-rules` flags the empty-`match` footgun and contradictory
  enable/disable-in-one-rule; counts rules correctly; exits non-zero on a
  missing `--rules-path`.
- ✅ Pattern matcher (`src/core/pattern.rs`, ~10 K lines) is a full-parity port
  of the firmware `pattern_match.c` (Thompson NFA; `* ^ $ WT + \d \D \w \W \s \S
  \b \B .`) with a ported test corpus.

### CLI + config/rules user journeys (README/docs)
- ✅ `--help` reports the Cargo.toml version; `-l`, `-c`, `--validate-rules`,
  `--list-devices` all dispatch.
- ✅ `-c` seeds a zero-config `config.toml` **and** `rules.toml`; idempotent
  (re-run does not overwrite); the template parses to all-default (inert).
- ✅ Seeded config contains **no literal `0xfeed`** (DEVICE_DISCOVERY.md §7.2
  cleanup) and uses the `0x????` auto-discovery hint.
- ✅ Hot-config: an explicit config (incl. `[linux]` table) round-trips through
  `render_config_body` (no field loss on a Settings save).

### Packaging integrity — actually built, not just metadata-checked
- ✅ `cargo deb` → `qmkonnect_0.2.8-1_amd64.deb` (960 KB); `data.tar.xz` carries
  all 6 FHS assets (`/usr/bin/qmkonnect`, `/usr/lib/udev/qmkonnect-hid-id`,
  `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules`,
  `/usr/lib/systemd/user/qmkonnect.service.template`,
  `/etc/xdg/autostart/qmkonnect.desktop`, doc); `control.tar.xz` embeds
  `postinst`/`prerm`/`postrm`.
- ✅ `cargo generate-rpm` → `qmkonnect-0.2.8-1.x86_64.rpm` (1.1 MB); maintainer
  scripts wired (`post_install_script`/`post_uninstall_script` ref + files).
- ✅ Both built **without** `-lhidapi-hidraw` (correct for unified-hidapi
  Debian/Fedora; ARCHITECTURE.md invariant 12).
- ✅ All community-channel manifests (Scoop, Winget×3, Homebrew cask, AUR + source
  PKGBUILD, GNOME extension `metadata.json`) carry version `0.2.8`.
- ✅ Both Arch PKGBUILDs install the XDG autostart `.desktop` (F17).

### Live product smoke (real keyboard attached)
- ✅ App starts under `-v`; `select_linux_backend` logs its probes and selects
  `foreign-toplevel`.
- ✅ **Tier-2 capability handshake succeeds against real hardware** —
  `proto v2 capable (flags=0x03, 1 callbacks, board_rules=true)`.
- ✅ **Window-detection → `notify_qmk` pipeline fires live** — detected the
  focused `Alacritty` window and sent the debounced payload
  `Alacritty\x1Dterminal - pi` plus the `ApplyHostContext` typed command.

### CI release pipeline
- ✅ `.github/workflows/release.yml` is comprehensive and well-documented:
  macOS (DMG, optional notarize), Windows (Inno), Linux binary tarball + Arch
  `.pkg.tar.zst`, GNOME extension zip, `.deb` + `.rpm` post-publish jobs, and
  AUR / Homebrew-tap / Scoop-bucket / Winget-PR / asdf-plugin publishing with
  their deploy-key/PAT secrets documented inline.

---

## Residual Risks (informational, not actionable as bugs)

1. **Binaries are unsigned/ad-hoc-signed** (PRD §12, by design). Winget shows
   "unverified publisher"; macOS Screen-Recording re-prompts on every rebuild
   until a stable Developer ID lands. Not a defect — documented beta status.
2. **Community-publish CI jobs require one-time secret/repo setup** (AUR SSH key,
   Homebrew/Scoop/asdf deploy keys, Winget PAT, winget-pkgs initial entry). The
   workflow comments document each; first-run "Permission denied (publickey)"
   until configured is expected.
3. **Proto-v1 firmware** (legacy qmk_notifier flash) can briefly reset the active
   board layer on a per-candidate picker probe (`classify_devices`), documented
   in DEVICE_DISCOVERY.md §2.2 — recovery is to reflash current firmware.

---

## Recommended Action Order

1. **🔴 H1** — `cargo fmt --all` + commit (1 min; unblocks the `main` CI gate).
2. **🟡 M1** — resolve the Nix `cargoHash` placeholder so the advertised Nix
   channel actually builds (one `nix build` iteration; then drop `--no-build`).
3. **🟠 M2** — refresh `print_platforms()` to reflect F16's multi-backend build
   (cosmetic).

*Generated by `./validate.sh` (run it with `--skip-live` to omit the 3 s
hardware smoke test, or as-is to exercise the live HID handshake).*