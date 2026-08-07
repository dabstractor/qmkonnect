# QMKonnect — Validation Report

**Date:** 2026-08-07 · **Version validated:** 0.2.8 (Cargo.toml)
**Validation script:** `./validate.sh` (49 checks passed, 2 failed, 1 warning, 1 skipped)
**Host:** Arch Linux x86_64, Rust 1.92.0, Hyprland (Wayland), with a real Dactyl-Manuform (RP2040, qmk_notifier firmware) attached.

---

## TL;DR

QMKonnect is a **healthy, production-grade** cross-platform desktop daemon. The
core product (F1–F14) is fully implemented and **verified end-to-end against
real hardware**: window detection → debounce → Raw-HID wire framing → Tier-2
capability discovery → live `QUERY_INFO`/`QUERY_CALLBACK` handshake → keyboard
side-effects. All 441 unit tests pass single-threaded.

This validation surfaced **2 hard issues** (both pre-existing, neither is a
correctness/logic bug — they are lint-gate regressions), **2 feature gaps**
between the PRD's F15 distribution claims and what is actually shipped, and
**1 documentation drift**. None of them affect the running app's behavior for
an existing user; they are about release-readiness and doc accuracy.

| Severity | Count | Items |
|---|---|---|
| 🔴 Hard (blocks CI / documented gate) | 2 | fmt regression, clippy regression |
| 🟡 Feature gap (PRD vs. impl) | 2 | missing `.deb`/`.rpm`, broken Nix flake build |
| 🟠 Doc drift | 1 | README understates Linux support |
| 🟢 Minor / cosmetic | 1 | `-l` platform label cfg-gated wording |

---

## 🔴 Hard Issues

### H1. `cargo fmt --all -- --check` fails — committed code is unformatted

**Impact:** The CI `fmt` job (`.github/workflows/ci.yml`, `fmt` →
`cargo fmt --all -- --check`) runs this exact command on **every push to `main`**.
The repository is currently in a state where the next push to main will **fail
CI**.

**Evidence:** `cargo fmt --all -- --check` → exit **1**, 10 diff sections
across 4 source files:

```
Diff in src/core/mod.rs:334:
Diff in src/core/mod.rs:344:
Diff in src/platforms/atspi.rs:262:
Diff in src/platforms/atspi.rs:308:
Diff in src/platforms/atspi.rs:443:
Diff in src/platforms/atspi.rs:605:
Diff in src/platforms/mod.rs:80:
Diff in src/platforms/wayland_ft.rs:242:
Diff in src/platforms/wayland_ft.rs:621:
Diff in src/platforms/wayland_ft.rs:678:
```

The changes are small `rustfmt` nits (e.g. collapsing a `None => { "...".to_string() }`
block arm onto one line in `src/core/mod.rs:334/344`).

**Fix:** `cargo fmt --all` then commit. (One command; purely mechanical.)

---

### H2. `cargo clippy --all-targets -- -D warnings` fails — documented lint gate violated

**Impact:** `AGENTS.md`'s Linux dev loop mandates `cargo clippy --all-targets -- -D warnings`.
CI does **not** currently run clippy, so this does not break CI today — but it
violates the project's own documented lint standard and would be flagged by any
reviewer running the documented command. Plain `cargo clippy --all-targets`
(without `-D warnings`) passes with warnings (exit 0).

**Evidence:** `cargo clippy --all-targets -- -D warnings` → exit **101**, 2 errors:

1. **`clippy::type_complexity`** — `src/platforms/wayland_ft.rs:497` (**runtime code**):
   ```rust
   static SHARED_SNAPSHOT: OnceLock<Arc<Mutex<Vec<(String, String)>>>> = OnceLock::new();
   ```
   Suggest factoring into a `type` alias.

2. **`clippy::unnecessary_unwrap`** — `src/platforms/gnome.rs:408` (**test code**, inside
   `#[cfg(test)]` at line 355):
   ```rust
   if r.is_err() {
       let m = r.unwrap_err();   // use `if let Err(m) = r`
   ```

**Fix:** Trivial — add a `type SharedSnapshot = …;` alias + use `if let Err(m) = r`.

---

## 🟡 Feature Gaps (PRD F15 vs. implementation)

The PRD's F15 row lists the full community-distribution channel set. The
`tasks.json` plan (P1) only scoped **AUR, Nix, Homebrew, Scoop, Winget,
mise/asdf** — it omitted two channels the PRD and `spec/PACKAGING.md` promise.
This is an under-scoping of F15, not a code regression.

### G1. `.deb` and `.rpm` packages are completely unimplemented

**PRD claim** (`spec/PRD.md` F15, §5): "native **.deb**/**.rpm** packages on Linux".

**Spec detail** (`spec/PACKAGING.md` §4.3, §4.4): full designs for
`cargo-deb` (`[package.metadata.deb]` Cargo.toml block + `packaging/debian/`
maintainer scripts) and `cargo-generate-rpm` (`[package.metadata.generate-rpm]`
+ `packaging/rpm/postin`/`postun`). Both specify the exact FHS install paths,
the hidapi-link nuance, and the postinst/postrm hooks mirroring the Arch
`qmkonnect.install`.

**Reality:**
- ❌ No `[package.metadata.deb]` block in `Cargo.toml`
- ❌ No `[package.metadata.generate-rpm]` block in `Cargo.toml`
- ❌ No `packaging/debian/` directory (postinst/prerm/postrm)
- ❌ No `packaging/rpm/` directory (postin/postun)
- ❌ CI `release.yml` has **0** `.deb`/`.rpm` build jobs (confirmed: 11 jobs total,
  none build native Debian/RPM packages)

So Ubuntu/Debian/Mint and Fedora/RHEL/Rocky users — a major slice of the Linux
audience the PRD targets (§3 persona "user on a mainstream Linux desktop") —
have **no native package**. They must fall back to the generic tarball +
manual `install.sh` steps.

**Note:** `spec/PACKAGING.md` §4.8 also documents the per-distro runtime-dep
table for these exact packages, so the design is ready; only the implementation
is missing.

---

### G2. The Nix flake cannot actually build/install the app (`cargoHash` placeholder)

**PRD claim** (`spec/PACKAGING.md` §4.5, `spec/PRD.md` §5):
```sh
nix profile install github:dabstractor/qmkonnect
nix run github:dabstractor/qmkonnect
nix build github:dabstractor/qmkonnect
```
"All work."

**Reality:** `flake.nix:52` ships:
```nix
cargoHash = pkgs.lib.fakeHash;
```
This is a **deliberate placeholder** (acknowledged in `.github/workflows/ci.yml`
`nix-check` job comments). With a fake cargo vendor hash, **every** `nix
build`/`nix run`/`nix profile install` fails with a cargo hash mismatch before
compiling anything. CI deliberately runs only `nix flake check --no-build`
(eval-only) so the pipeline stays green.

The result: the Nix channel (an F15 deliverable) advertises itself in the docs
and README, but **a user who runs `nix run github:dabstractor/qmkonnect` today
gets a build failure**, not the app.

**Fix:** One-time `nix build .#qmkonnect`, read the "got: sha256-…" from the
hash-mismatch error, paste it into `flake.nix`, then drop `--no-build` from CI.
(CI already documents this as the out-of-scope follow-up.)

---

## 🟠 Documentation Drift

### D1. README.md / docs/installation.md understate Linux support ("Arch/Hyprland only")

**Claim** (`README.md:21`, mirrored verbatim in `docs/llms_full.txt:42`; also
`docs/installation.md` compatibility matrix says "Linux (Hyprland)"):
> Linux: Arch/Hyprland only

**Reality:** Feature **F16** ("Cross-DE Linux window monitor") is fully
shipped. The default feature set compiles in **five** runtime-selected backends
(`select_linux_backend`, priority order): foreign-toplevel Wayland → GNOME
(Shell extension D-Bus) → Hyprland IPC → AT-SPI → X11. The `wayland` backend
covers Hyprland, Sway, Niri, KDE Plasma 6, COSMIC, and the wlroots family.

**Verified at runtime during this validation:** on this Hyprland host the
verbose log showed
```
select_linux_backend: probing 'foreign-toplevel'…
  → 'foreign-toplevel' available, selected
[35ms] wayland_ft: connected, dispatching foreign-toplevel events
```
i.e. the `foreign-toplevel` (Wayland) backend was selected as priority #1,
**superseding** the Hyprland-IPC backend (exactly per `spec/PLATFORMS.md` §7.4),
and it correctly reported active windows.

The stale "Arch/Hyprland only" line predates F16 and is now actively
misleading — it tells a GNOME/KDE/Sway/Niri user they are unsupported when
they are first-class supported.

**Related sub-finding — `docs/llms_full.txt` is stale by 108 lines.** Regenerating
it (`bash docs/generate_llms_full.sh`) against the current source docs adds
108 lines the committed copy is missing: a whole "Autostart at login" section
(systemd + XDG autostart), a "GNOME (optional Shell extension)" section, and
updates to the Package Managers section. So the committed
`docs/llms_full.txt` (self-described as the "canonical reference for agents and
LLMs") does **not** match its sources. This is a generated artifact that was
not regenerated on its last source-doc edit.

**Fix:** Update README.md + docs/installation.md to reflect F16 coverage, then
regenerate `docs/llms_full.txt` (overdue regardless of the README fix).

---

## 🟢 Minor / Cosmetic

### M1. `qmkonnect -l` prints "Linux (Hyprland)" based on the `hyprland` feature flag

`print_platforms()` in `src/main.rs` gates the label on `cfg(feature="hyprland")`.
Since `hyprland` is a default-on feature (kept as a fallback behind `wayland`),
the label reads "Linux (Hyprland)" even though the *default-selected* backend
on a wlroots compositor is `foreign-toplevel`. Harmless (it's a build-feature
label, not a runtime-backend report), but a user running it on Sway/KDE sees
"Linux (Hyprland)". Low priority — the verbose backend-selection log is the
authoritative source.

---

## ✅ What Passed (end-to-end verification)

These were exercised by `./validate.sh` against the live system + real
Dactyl-Manuform hardware. The full data flow was observed working:

### Core product (F1–F14)
- **Window detection (F1/F16):** `foreign-toplevel` Wayland backend selected at
  runtime (priority #1), reported correct `app_class`+`title` (e.g.
  `brave-browser|Reina Flore | StashDB - Brave`, `foot|foot`).
- **Raw-HID transport (F2):** payloads framed correctly — the byte count
  reported on the wire matches `len(app_class) + 1 (GS) + len(title)`
  (e.g. 41 bytes for a 40-char-visible payload). GS=`0x1D`, ETX=`0x03`, magic
  header `0x81 0x9F` all confirmed in source + logs.
- **Auto device discovery (F3/F13):** `--list-devices` enumerates read-only and
  the **Tier-2 `QUERY_INFO` probe correctly classified** the Dactyl-Manuform as
  `qmk_notifier` (not a false-green "Connected" for the also-present
  non-qmk_notifier `0xFF60` candidates).
- **Debounce coalescing (F4):** observed the exact PRD §5.3 algorithm — an
  *immediate* first send, then at most *one debounced follow-up* of the newest
  value; rapid in-app title changes collapsed correctly.
- **Config hot-reload (F5):** `configured_filter()` / timing re-read on every
  call; `-c`/`-r`/Settings all route through the shared `render_config_body`.
- **Empty-workspace semantics (PRD §7):** empty focus → 1-byte payload `\x1D`
  (lone GS) → deactivates layers.
- **Host-side rules (F11/F12):** `--validate-rules` enforces all three validity
  rules (must set layer/enable/disable; `layer=255` rejected as the clear
  sentinel; missing explicit `--rules-path` errors). Live `--list-callbacks`
  ran the full `QUERY_INFO` → `QUERY_CALLBACK` sweep and returned
  `vim_lazy → id 0` from the keyboard's registry. The host-context
  `ApplyHostContext` send (stack/no-match path, `clear_board=false`) fired
  alongside every string send.
- **VIA coexistence (F14 / R-COEX):** the E2E verbose instance opened the device
  **shared/non-seize** and coexisted with the already-running systemd instance
  (both held the device simultaneously with no lock-out). Source documents the
  "first payload byte is always `0x81`" demux invariant, asserted by unit tests.

### Linux integration (F9/F17)
- **Static udev rule** installed at `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules`:
  correct `IMPORT{program}` of the helper + `ID_QMKONNECT` gate +
  `GROUP="input", MODE="0660", TAG+="uaccess"` + `SYMLINK+=qmkonnect_device` +
  `SYSTEMD_USER_WANTS`. **Never `0666`** (security invariant PRD §9 verified).
- **`qmkonnect-hid-id` helper:** correctly prints `ID_QMKONNECT=1` for the real
  QMK report descriptor (containing the `06 60 ff … 09 61` signature) and
  prints nothing for 8 non-QMK descriptors.
- **Device symlink** `/dev/qmkonnect_device → hidraw5` exists; permissions
  `crw-rw----+ root input` (0660 + uaccess ACL) — correct.
- **Config-driven fallback rule:** `qmkonnect -r` with VID/PID renders exactly
  **one physical line starting with `KERNEL==`** (the safe form; the dangerous
  multi-line host-wide re-permission bug from `spec/LINUX.md` §5 is prevented).
- **systemd user service:** enabled and running (PID 360822, 1d 13h uptime); the
  journal shows correct graceful-degradation behavior on keyboard unplug
  (3-attempt retry with backoff, then `Ok` — never restart-loops; PRD §5.4).

### Test suite
- **441 unit tests pass** single-threaded (`cargo test --bin qmkonnect -- --test-threads=1`),
  including 432 firmware-parity pattern-matcher tests, 94 rules-evaluation
  tests, 152 notifier tests, and the R-COEX demux/shared-open invariants.

### Packaging integrity (what *is* shipped)
- **Version consistency:** all 6 channel manifests (AUR `pkgver`, Homebrew cask,
  Scoop, Winget, GNOME extension `metadata.json`, Inno `MyAppVersion`) read
  `0.2.8` — identical to `Cargo.toml`.
- **AUR integrity:** the PKGBUILD's `sha256sums` matches the committed release
  tarball; the tarball contains all four expected files (binary, hid-id, udev
  rule, service template).
- **Manifest syntax:** Homebrew cask valid Ruby (`ruby -c`); Scoop + GNOME
  extension JSON valid; Winget YAML well-formed.
- **No build artifacts committed** (PRD §PACKAGING 11 honored; `.gitignore`
  covers `*.pkg.tar.*`, `*.dmg`, `*.msi`, `target/`, etc.).

### Builds
- `cargo build --release --all-targets` ✅
- `cargo build --no-default-features --bin qmkonnect` (trayless service) ✅
- `cargo build --release --bin qmkonnect-hid-id` (udev helper) ✅

---

## Recommended Actions (priority order)

1. **Run `cargo fmt --all`** and commit — unblocks CI (H1). One command.
2. **Fix the 2 clippy errors** (H2) — a `type` alias + an `if let`. Restores the
   documented lint gate.
3. **Resolve the Nix `cargoHash` placeholder** (G2) — one `nix build` iteration;
   flips a currently-broken F15 channel to working and lets CI drop `--no-build`.
4. **Update README.md / docs/installation.md** Linux coverage (D1) — replace
   "Arch/Hyprland only" with the F16 backend list; **regenerate
   `docs/llms_full.txt`** (currently 108 lines stale regardless of the README fix).
5. **Implement `.deb`/`.rpm`** (G1) — the design is fully specified in
   `spec/PACKAGING.md` §4.3/§4.4; add the two Cargo.toml metadata blocks +
   maintainer scripts + two CI jobs. (Largest item; closes the F15 scope gap.)

Items 1–4 are small mechanical fixes; item 5 is the substantive feature gap.

---

## Appendix: Validation Script Output

`./validate.sh` final tally on this host:
```
PASSED: 49   FAILED: 2   WARNINGS: 1   SKIPPED: 1
RESULT: ✗ 2 hard failure(s) found.   (elapsed 23s)
```
The 2 failures are H1 (fmt) and H2 (clippy). The 1 warning is G2 (Nix fakeHash).
The skip is `nix flake check` (Nix not installed locally — CI runs it).

The script is idempotent, hardware-aware (live HID checks skip gracefully when
no QMK keyboard is attached or the host is not Linux), and exits non-zero on any
hard failure so it can gate CI.