# PRP — P2.M1.T2.S2: Add wayland/gnome/atspi Cargo features, X11 unconditional, hidapi link nuance

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Files
> edited:** `Cargo.toml` (the `[features]` block + the
> `[target.'cfg(target_os = "linux")'.dependencies]` block + a documentation comment
> near `hidapi`), and `.cargo/config.toml` (a documentation comment — NO new rustflag).
> **No source-code changes** beyond an X11 *verification* (it is already unconditional
> in the tree — see §What/GOTCHA-1). No new backend modules (`wayland_ft.rs` / `gnome.rs`
> / `atspi.rs` are P2.M2 / P2.M3 / P2.M4). No `docs/*` prose (Mode A: the feature table
> in `spec/PACKAGING.md` §2 is the reference; Cargo.toml comments stay in sync with it).
>
> **What this does:** declares the three new Linux window-monitor backend Cargo
> features (`wayland`, `gnome`, `atspi`) and their four optional dependencies
> (`smithay-client-toolkit`, `wayland-client`, `zbus`, `atspi`), adds them to `default`
> (so a single binary works on every Linux desktop per F16/PRD §4), confirms X11 is
> compiled unconditionally (no feature gate — runtime-gated on `$WAYLAND_DISPLAY`
> instead, ARCHITECTURE invariant 11), and documents the per-distro hidapi link
> nuance (ARCHITECTURE invariant 12) in-repo so the `.deb`/`.rpm`/Nix builds don't
> break usage/usage_page matching. This is the **build-features half** of F16; the
> dispatcher half is P2.M1.T1.S1 (parallel) and the config-schema half is
> P2.M1.T2.S1 (parallel).
>
> **Source of truth:** `spec/PACKAGING.md` §2 (the authoritative feature table —
> quoted verbatim in `research/notes.md` §1), `spec/ARCHITECTURE.md` invariant 12
> (the hidapi nuance) + invariant 11 (X11-not-under-Wayland), and the production
> code itself (`Cargo.toml`, `.cargo/config.toml`, `src/platforms/{mod,x11,linux}.rs`
> — verified current state in `research/notes.md` §2–§4).

---

## Goal

**Feature Goal**: Bring `Cargo.toml`'s `[features]` + Linux target deps into
parity with the canonical feature table in `spec/PACKAGING.md` §2: add
`wayland = ["dep:smithay-client-toolkit", "dep:wayland-client"]`,
`gnome = ["dep:zbus"]`, and `atspi = ["dep:atspi"]` to `[features]`, add all
three to `default`, and declare the four crates as `optional = true` under
`[target.'cfg(target_os = "linux")'.dependencies]`. Confirm X11 stays compiled
unconditionally on Linux (no feature gate). Document the hidapi link nuance
(Arch links `-lhidapi-hidraw`; unified-hidapi distros — Debian/Ubuntu/Fedora +
modern Nixpkgs — must NOT) in `Cargo.toml` and `.cargo/config.toml` so the
future `.deb`/`.rpm` (P1.M7) builds and any agent touching hidapi see it.

**Deliverable** (concrete; compiles + passes tests on the dev box TODAY):
- `Cargo.toml` `[features]` block rewritten to the §2 target (`default` includes
  `wayland`, `gnome`, `atspi`, `hyprland`, `macos`, `linux-tray`); three new
  feature lines using `dep:` syntax (matching the existing `hyprland`/`macos`/
  `linux-tray` style).
- `Cargo.toml` `[target.'cfg(target_os = "linux")'.dependencies]` block gains
  four `optional = true` lines: `smithay-client-toolkit = { version = "0.20",
  optional = true }`, `wayland-client = { version = "0.31", optional = true }`,
  `zbus = { version = "5", optional = true }`, `atspi = { version = "0.30",
  optional = true }`.
- `Cargo.toml` gains a documentation comment near `hidapi = "2.6"` stating the
  per-distro link nuance + pointing to `spec/PACKAGING.md` §2.
- `.cargo/config.toml` gains a documentation comment explaining the INTENTIONAL
  absence of a global `-lhidapi-hidraw` rustflag (preempting an agent adding one).
- `src/platforms/x11.rs` + `src/platforms/mod.rs`: VERIFIED unchanged (X11 already
  target-gated only — GOTCHA-1). If a stray feature gate is found, remove it;
  otherwise touch nothing.

**Success Definition**:
- `cargo build --bin qmkonnect` (default features) compiles clean — the four new
  crates resolve + build (this now pulls `libwayland` via `wayland-sys` on the
  build host; see GOTCHA-3).
- `cargo build --bin qmkonnect --no-default-features` (minimal trayless build)
  compiles clean — the four new crates are absent from the graph (proving they
  are genuinely optional + `dep:`-gated).
- `cargo test --bin qmkonnect -- --test-threads=1` passes (no regression; the
  feature-gated probe stubs in `select_linux_backend` become live and return
  graceful `Err("…not yet implemented (P2.M2/P2.M3/P2.M4)")` — by design).
- `cargo build --bin qmkonnect --features wayland` / `--features gnome` /
  `--features atspi` each individually toggle their deps on (smoke check the
  feature wiring).
- `grep -rn 'feature.*x11\|x11.*feature' src/` returns nothing (X11 ungated).
- `git diff --stat` shows ONLY `Cargo.toml` + `.cargo/config.toml` (no source
  files unless a stray X11 gate needed removal, no `docs/*`, no PRD/tasks.json,
  no `select_linux_backend` edits — that's P2.M1.T1.S1).

## User Persona (if applicable)

**Target User**: (a) The downstream backend implementers (P2.M2 `wayland_ft.rs`,
P2.M3 `gnome.rs`, P2.M4 `atspi.rs`) who `use smithay_client_toolkit` / `zbus` /
`atspi` behind their feature gates; and (b) distro packagers / the release CI
that build QMKonnect with `default` features (full multi-DE binary) or a trimmed
feature set.

**Use Case**: A packager runs `cargo build --release` (default) to produce one
binary that auto-selects its window monitor across GNOME/KDE/COSMIC/Hyprland/
Sway/Niri/wlroots/X11 at runtime. A minimal-service builder runs
`cargo build --no-default-features` to drop every optional Linux backend dep.

**Pain Points Addressed**: Makes the F16 cross-DE backends *selectable + default-
on* at the Cargo level (today the features don't exist, so
`select_linux_backend`'s `#[cfg(feature = "wayland")]` / `gnome` / `atspi` probe
stubs are compiled OUT and the backends can't be turned on). Documents the one
distro-specific link trap (hidapi) that, if a packager gets it wrong, silently
breaks usage/usage_page device matching on `.deb`/`.rpm`.

## Why

- **F16 (PRD §4) requires a single binary that works on every Linux desktop.**
  `spec/PACKAGING.md` §2 mandates `default = ["wayland", "gnome", "atspi",
  "hyprland", "macos", "linux-tray"]` so plain `cargo build --release` yields
  the full multi-DE app. This task IS the Cargo.toml half of that; the backends
  themselves are P2.M2–P2.M4.
- **The dispatcher already expects these features.** `select_linux_backend`
  (P2.M1.T1.S1, in the tree) gates its wayland/gnome/atspi probe stubs on
  `#[cfg(feature = "wayland")]` / `gnome` / `atspi`. Today those features are
  undefined, so the stubs are dead. Declaring the features makes the stubs live
  (returning graceful "not yet implemented" errors) and prepares the contract
  surface the backend tasks replace.
- **X11 must stay unconditional.** ARCHITECTURE invariant 11 forbids selecting
  X11 under a Wayland compositor; that gate lives at RUNTIME
  (`$WAYLAND_DISPLAY` unset in `select_linux_backend`), not compile-time. Making
  X11 a feature would either drop the universal fallback or require every build
  to remember a flag. X11 is already unconditional in the tree — this task
  verifies that and documents why.
- **The hidapi link nuance is a silent-failure trap.** Arch's separate
  `libhidapi-hidraw` vs Debian/Fedora's unified `libhidapi` (≥0.14): getting the
  link flag wrong breaks usage/usage_page device matching with no compile error.
  The `.deb`/`.rpm` packaging (P1.M7) does not exist yet, so documenting the
  nuance in-repo now (Cargo.toml + `.cargo/config.toml` comments) is the only way
  the P1.M7 agent inherits it.

## What

Four edits, each independently verifiable:

### (a) `[features]` block → §2 target
Rewrite to:
```toml
[features]
default = ["wayland", "gnome", "atspi", "hyprland", "macos", "linux-tray"]
# Linux window-monitor backends (runtime-selected by select_linux_backend,
# PLATFORMS.md §6). All default-on so a single binary works everywhere;
# turn a backend off to shrink the binary / drop a dep (spec/PACKAGING.md §2).
wayland    = ["dep:smithay-client-toolkit", "dep:wayland-client"]  # foreign-toplevel (Hyprland/Sway/Niri/wlroots/KDE/COSMIC)
gnome      = ["dep:zbus"]                                           # GNOME Shell-extension D-Bus client
atspi      = ["dep:atspi"]                                          # a11y-bus fallback
hyprland   = ["dep:hyprland"]                                       # legacy Hyprland-IPC backend (superseded by wayland)
linux-tray = ["dep:ksni", "dep:gtk"]                                # StatusNotifierItem tray
macos      = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
```
Use `dep:` syntax (NOT bare crate names) — matches existing features and avoids
implicit same-named features (GOTCHA-7).

### (b) Linux target deps → add four `optional = true`
Add under `[target.'cfg(target_os = "linux")'.dependencies]`:
```toml
# F16 cross-DE backends (P2.M2/P2.M3/P2.M4 implement the modules; the features
# are declared here so select_linux_backend's #[cfg(feature=…)] probe stubs are
# selectable). All optional + dep:-gated so --no-default-features drops them.
# MSRV note: smithay-client-toolkit 0.20 MSRV is 1.86 (< project floor 1.88 ✓).
smithay-client-toolkit = { version = "0.20", optional = true }
wayland-client         = { version = "0.31", optional = true }   # matches SCTK 0.20's wayland-client major
zbus                   = { version = "5",   optional = true }    # GNOME D-Bus client; atspi 0.30 also uses zbus 5 (unified)
atspi                  = { version = "0.30", optional = true }   # pure-Rust AT-SPI2 (odilia-dev)
```
Keep the existing `hyprland` / `libxdo` / `tempfile` / `libc` / `ksni` / `gtk`
lines untouched.

### (c) X11 unconditional — VERIFICATION (no edit unless a stray gate is found)
Confirm — DO NOT add a feature gate:
1. `src/platforms/x11.rs:1` is `#![cfg(target_os = "linux")]` (target gate only).
2. `src/platforms/mod.rs` declares `#[cfg(target_os = "linux")] mod x11;` (the
   explanatory comment is already present).
3. `src/platforms/linux.rs` `select_linux_backend` adds the `x11` candidate
   unconditionally (always last; runtime-gated on `$WAYLAND_DISPLAY` unset).
4. `grep -rn 'feature.*x11\|x11.*feature' src/` returns nothing.
All three are already true in the tree. **If and only if** a stray `feature`
gate on X11 is found, remove it so X11 is target-gated only. Otherwise: no edit.

### (d) hidapi link nuance — documentation comments
Add a comment near `hidapi = "2.6"` in `[dependencies]` AND a comment in
`.cargo/config.toml` explaining the intentional absence of a global
`-lhidapi-hidraw` rustflag. Exact text in the Implementation Blueprint. (NO new
rustflag is added — GOTCHA-4: a global flag would break `.deb`/`.rpm`/Nix
unified-hidapi builds.)

### Success Criteria
- [ ] `[features] default` includes `wayland`, `gnome`, `atspi` (in addition to `hyprland`, `macos`, `linux-tray`).
- [ ] `wayland = ["dep:smithay-client-toolkit", "dep:wayland-client"]`, `gnome = ["dep:zbus"]`, `atspi = ["dep:atspi"]` exist with `dep:` syntax.
- [ ] The four crates are declared `optional = true` under `[target.'cfg(target_os = "linux")'.dependencies]`.
- [ ] `cargo build --bin qmkonnect` (default) compiles — the four new crates build.
- [ ] `cargo build --bin qmkonnect --no-default-features` compiles — the four crates are absent (genuinely optional).
- [ ] `cargo build --bin qmkonnect --features wayland` / `--features gnome` / `--features atspi` each toggle their deps on.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes (no regression).
- [ ] `grep -rn 'feature.*x11\|x11.*feature' src/` returns nothing (X11 ungated).
- [ ] A comment documenting the hidapi link nuance is present in `Cargo.toml` (near `hidapi`) AND in `.cargo/config.toml` (near the existing rustflags / a new `[target.'cfg(target_os = "linux")']` comment-only stanza).
- [ ] `.cargo/config.toml` has NO `-lhidapi-hidraw` rustflag (only the Windows `+crt-static` one remains).
- [ ] `git diff --stat` shows ONLY `Cargo.toml` + `.cargo/config.toml` (no source edits unless a stray X11 gate was removed).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior knowledge can implement this using only this PRP +
the repo, because (a) the AUTHORITATIVE target feature table is quoted verbatim from
`spec/PACKAGING.md` §2 in `research/notes.md` §1 (exact TOML to transcribe), (b) the
verified CURRENT `Cargo.toml` state (the BEFORE) is recorded in §2 so the agent knows
exactly what to change, (c) part (c) is shown to be ALREADY done with the three
verification sites + the grep command, (d) the hidapi nuance is pinned to exact
file:line evidence (§4) with the exact comment text to write, (e) the four crate
versions + MSRV compatibility are decided (§5) so the agent doesn't guess, (f) the
two load-bearing build-time gotchas (default-build compiles the new crates; no global
hidapi rustflag) are explicit. See `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the authoritative feature table (the spec to transcribe)
- file: spec/PACKAGING.md
  why: "§2 'Features & Binaries' is the canonical [features] block (quoted verbatim
        in research/notes.md §1): default includes wayland/gnome/atspi; the exact
        dep: wiring for each; AND the 'Linux hidapi link nuance (must-preserve)'
        paragraph that part (d) documents. §4.3/§4.4/§4.8 give the per-distro
        build/link rules (.deb/.rpm build WITHOUT -lhidapi-hidraw; Arch WITH)."
  pattern: "[features] default=[...]; wayland=['dep:smithay-client-toolkit',
            'dep:wayland-client']; gnome=['dep:zbus']; atspi=['dep:atspi']; plus
            the existing hyprland/linux-tray/macos lines unchanged."
  gotcha: "the hidapi nuance paragraph in §2 is the single source of truth for part
           (d); invariant 12 in ARCHITECTURE.md restates it."

# MUST READ — the invariants this task preserves
- file: spec/ARCHITECTURE.md
  why: "invariant 11 (line ~462: 'Never select the X11 backend under a Wayland
        compositor' — enforced at RUNTIME via $WAYLAND_DISPLAY, NOT a feature gate
        ⇒ X11 stays unconditional, GOTCHA-1) and invariant 12 (line ~464: 'Hidapi
        link differs per distro' — Arch links -lhidapi-hidraw; Debian/Ubuntu/Fedora
        must NOT). Both are the contract this task honors."
  section: "## 10. Key Invariants a Dev Agent Must Preserve" (invariants 11 + 12)

# MUST READ — the file THIS task edits (verified current state in notes §2)
- file: Cargo.toml
  why: "the [features] block (currently default=['hyprland','macos','linux-tray'],
        no wayland/gnome/atspi) + [target.'cfg(target_os=\"linux\")'.dependencies]
        (currently hyprland/libxdo/tempfile/libc/ksni/gtk — none of the 4 new
        crates) + the hidapi='2.6' line in [dependencies] (documentation anchor).
        EDIT: rewrite [features], add 4 optional deps, add the hidapi comment."
  pattern: "existing features use dep: syntax (hyprland=['dep:hyprland']); linux
            deps use { version=…, optional=true } for feature-gated crates (hyprland,
            ksni, gtk). Match that shape for the 4 new crates."
  gotcha: "rust-version='1.88' is the MSRV floor. smithay-client-toolkit 0.20 MSRV
           is 1.86 (< 1.88 ✓). Keep dep: syntax; do NOT use bare crate names."

# MUST READ — the other file THIS task edits
- file: .cargo/config.toml
  why: "currently ONLY the Windows MSVC +crt-static rustflag stanza. ADD: a comment
        (NOT a rustflag) explaining why there is intentionally no global
        -lhidapi-hidraw. Do NOT add any rustflag (GOTCHA-4)."
  gotcha: "a global [target.'cfg(target_os=\"linux\")'] rustflags=[…'-lhidapi-hidraw']
           would be inherited by EVERY Linux build incl. future .deb/.rpm/Nix and
           break unified-hidapi distros. The flag stays per-channel (env RUSTFLAGS
           in the Arch PKGBUILD, Nix hidrawFlag)."

# MUST READ — the file that ALREADY references these features (no edit needed here)
- file: src/platforms/linux.rs
  why: "select_linux_backend (P2.M1.T1.S1, in the tree) already gates its probe
        stubs on #[cfg(feature='wayland')] / 'gnome' / 'atspi' (lines ~47/52/62)
        and adds the x11 candidate UNCONDITIONALLY (line ~67). Once THIS task
        declares the features, those stubs become live (graceful Err) — by design.
        DO NOT edit this file (P2.M1.T1.S1 owns it)."
  pattern: "the probe stubs (wayland_probe/gnome_probe/atspi_probe at ~174-183)
            return Err('…not yet implemented (P2.M2/P2.M3/P2.M4)'). After this task
            they compile under default features and select_linux_backend falls
            through them to hyprland→x11. No regression."

# MUST READ — the X11 file (verification target, NO edit unless a stray gate exists)
- file: src/platforms/x11.rs
  why: "line 1 is #![cfg(target_os='linux')] — a TARGET gate, NOT a feature gate.
        Confirm this + that mod.rs gates the module by target_os only. Part (c) is
        a NO-OP verification (GOTCHA-1)."
  pattern: "the only cfg attrs in x11.rs are #![cfg(target_os='linux')] (line 1) and
            #[cfg(test)] (line 242). No feature gates."

# Reference — exact current packaging sites that DO pass the flag (do NOT change)
- file: packaging/linux/arch/PKGBUILD
  why: "line 25: RUSTFLAGS='-C link-arg=-lhidapi-hidraw' cargo build --release.
        This is CORRECT for Arch (separate libhidapi-hidraw). Do NOT remove it.
        (Companion: packaging/linux/aur/PKGBUILD:8, flake.nix:38/55/97 hidrawFlag.)"
- file: flake.nix
  why: "lines 32-38/55/97: hidrawFlag='-C link-arg=-lhidapi-hidraw' with a documented
        fallback (packaging/nix/README.md:115-119) to DROP it if Nixpkgs ships unified
        hidapi. This per-channel approach is the model — DO NOT globalize it."

# Reference — the parallel sibling PRP (config schema; do NOT duplicate its work)
- file: plan/007_fb356ba503b4/P2M1T2S1/PRP.md
  why: "the PARALLEL item adds LinuxConfig + [linux] table to src/core/mod.rs and
        closes the create_monitor TODO. It does NOT touch Cargo.toml. THIS task
        (P2.M1.T2.S2) owns Cargo.toml exclusively; do NOT overlap. Both land
        together to complete F16's config + build-features halves."

# Reference — crate versions (decided in research/notes.md §5; MSRV-compatible)
- url: https://crates.io/crates/smithay-client-toolkit
  why: "confirms smithay-client-toolkit 0.20 is current, MSRV 1.86 (< project 1.88 ✓).
        Sits on wayland-client 0.31."
- url: https://crates.io/crates/atspi
  why: "confirms atspi 0.30 is current; pure-Rust, zbus-based. Shares zbus 5 with
        the gnome feature (no two-major conflict — GOTCHA-5)."
```

### Current Codebase tree (relevant subset)

```bash
Cargo.toml                # [features] + [target.linux].dependencies + hidapi line  ← EDIT (a/b/d)
.cargo/config.toml        # Windows +crt-static rustflag only                       ← EDIT (d comment only)
src/
  core/mod.rs             # P2.M1.T2.S1 adds LinuxConfig here (parallel) — DO NOT TOUCH
  platforms/
    mod.rs                # `#[cfg(target_os="linux")] mod x11;` already present    ← VERIFY (c)
    linux.rs              # select_linux_backend (P2.M1.T1.S1) already has the      ← NO EDIT
                           #   #[cfg(feature="wayland"/"gnome"/"atspi")] probe stubs
    x11.rs                # `#![cfg(target_os="linux")]` already (no feature gate)  ← VERIFY (c)
    hyprland.rs           # unchanged
packaging/linux/arch/PKGBUILD  # KEEPS -lhidapi-hidraw (correct for Arch)           ← NO EDIT
flake.nix                      # hidrawFlag + documented fallback                    ← NO EDIT
spec/PACKAGING.md              # §2 feature table + hidapi nuance (source of truth) ← READ ONLY
spec/ARCHITECTURE.md           # invariants 11 (X11) + 12 (hidapi)                  ← READ ONLY
```

### Desired Codebase tree (files this task changes)

```bash
Cargo.toml                # MODIFIED — [features] rewritten; +4 optional linux deps; +hidapi comment
.cargo/config.toml        # MODIFIED — +documentation comment (NO new rustflag)
# (src/platforms/x11.rs + mod.rs touched ONLY if a stray X11 feature gate is found —
#  none exists today, so expect zero source edits)
```

### Known Gotchas of our codebase & Library Quirks

```toml
# CRITICAL (GOTCHA-1 — X11 is ALREADY unconditional; part c is VERIFICATION):
#   x11.rs:1 is #![cfg(target_os="linux")]; mod.rs gates the module by target_os;
#   select_linux_backend adds the x11 candidate UNGATED (runtime-gated on
#   $WAYLAND_DISPLAY unset instead — ARCHITECTURE invariant 11). Confirm with
#   `grep -rn 'feature.*x11\|x11.*feature' src/` (must be empty) and DO NOT add a
#   feature gate. Making X11 a feature would drop the universal fallback.
#
# CRITICAL (GOTCHA-2 — default-build now compiles the 4 new crates): adding
#   wayland/gnome/atspi to `default` means a plain `cargo build` resolves + builds
#   smithay-client-toolkit/wayland-client/zbus/atspi EVEN THOUGH no source uses
#   them yet (the backends are P2.M2-P2.M4). Cargo does not skip enabled optional
#   deps for being unused. This is the spec'd state (PACKAGING.md §2: "a single
#   binary works everywhere"). Verify `cargo build` succeeds; do NOT "optimize"
#   by leaving them out of default.
#
# CRITICAL (GOTCHA-3 — wayland pair needs libwayland): wayland-client +
#   smithay-client-toolkit link libwayland via wayland-sys. If `cargo build` fails
#   on a wayland-sys link/bindgen step, install libwayland-dev (Debian) /
#   wayland-devel (Fedora) / wayland (Arch). zbus + atspi are pure Rust (no system
#   build dep). Escape hatch: `cargo build --no-default-features --features macos,linux-tray`.
#
# CRITICAL (GOTCHA-4 — NO global hidapi rustflag): never add a
#   [target.'cfg(target_os="linux")'] rustflags=[…"-lhidapi-hidraw"] to
#   .cargo/config.toml. It would be inherited by EVERY Linux build (future .deb/.rpm
#   via cargo-deb/cargo-generate-rpm, and Nix) and break unified-hidapi distros
#   (Debian/Ubuntu/Fedora, modern Nixpkgs) where libhidapi-hidraw.so doesn't exist
#   OR silently mis-links the backend. The flag stays PER-CHANNEL: env RUSTFLAGS in
#   the Arch PKGBUILD, Nix hidrawFlag. Document the intentional absence with a
#   COMMENT in .cargo/config.toml, not a rustflag.
#
# GOTCHA-5 (zbus major must match atspi): gnome uses zbus directly; atspi 0.30 also
#   uses zbus. Both on zbus 5 today ⇒ cargo unifies them. If you bump one, bump the
#   other to the same major or cargo errors with a two-major conflict.
#
# GOTCHA-6 (MSRV): project rust-version=1.88. smithay-client-toolkit 0.20 MSRV 1.86
#   (< 1.88 ✓). A future SCTK bump above 1.88 forces a project MSRV bump — note it.
#
# GOTCHA-7 (use dep: syntax): `wayland = ["dep:smithay-client-toolkit", ...]`, NOT
#   `wayland = ["smithay-client-toolkit", ...]`. Bare-name creates an implicit
#   same-named feature AND enables the dep; dep: enables ONLY the dep. Existing
#   hyprland/linux-tray/macos all use dep: — match them. PACKAGING.md §2 uses dep:.
#
# GOTCHA-8 (scope): edit Cargo.toml + .cargo/config.toml ONLY. Do NOT create backend
#   modules (P2.M2-P2.M4), do NOT edit select_linux_backend (P2.M1.T1.S1) or
#   LinuxConfig (P2.M1.T2.S1). The probe stubs in linux.rs already exist and become
#   live once features are declared — no edit there.
```

## Implementation Blueprint

### Data models and structure

No data models — this is a build-configuration task. The "structure" is the
`[features]` table and the target-gated `[dependencies]` table in `Cargo.toml`,
plus the `.cargo/config.toml` rustflag/comment stanzas.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: BASELINE — confirm the build is green BEFORE any edit
  - RUN: cargo build --bin qmkonnect            (default features; must be green today)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1   (must pass today)
  - WHY: establishes that any later breakage is THIS task's fault, not pre-existing.
  - IF the baseline already fails: STOP and report (do not build on a red baseline).

Task 2: EDIT Cargo.toml [features] block (part a)
  - REPLACE the current:
        [features]
        default = ["hyprland", "macos", "linux-tray"]
        hyprland = ["dep:hyprland"]
        macos = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]
        linux-tray = ["dep:ksni", "dep:gtk"]
    WITH the §2 target (see "### (a)" above): default gains wayland/gnome/atspi;
    add the three new feature lines with dep: syntax (GOTCHA-7); keep hyprland/
    linux-tray/macos unchanged.
  - PRESERVE: the existing block-comment above [features] (the tray/default rationale)
    OR extend it to mention the new backends — keep it in sync with PACKAGING.md §2.

Task 3: EDIT Cargo.toml [target.'cfg(target_os = "linux")'.dependencies] (part b)
  - ADD four optional deps (place them near the existing `hyprland` optional line,
    grouped as the F16 backend deps — see "### (b)" above for exact text + comments):
        smithay-client-toolkit = { version = "0.20", optional = true }
        wayland-client         = { version = "0.31", optional = true }
        zbus                   = { version = "5",   optional = true }
        atspi                  = { version = "0.30", optional = true }
  - DO NOT touch: hyprland / libxdo / tempfile / libc / ksni / gtk lines.
  - VERSIONS are decided (research/notes.md §5): SCTK 0.20 MSRV 1.86 (< 1.88 ✓);
    wayland-client 0.31 matches SCTK 0.20; zbus 5 + atspi 0.30 share zbus 5 (GOTCHA-5).

Task 4: VERIFY X11 is unconditional (part c) — NO edit expected
  - RUN: grep -rn 'feature.*x11\|x11.*feature' src/   (expect: no output)
  - EYEBALL: src/platforms/x11.rs:1 == `#![cfg(target_os = "linux")]` (no feature gate)
  - EYEBALL: src/platforms/mod.rs has `#[cfg(target_os = "linux")] mod x11;`
  - EYEBALL: src/platforms/linux.rs select_linux_backend adds the `x11` candidate
    unconditionally (runtime-gated on $WAYLAND_DISPLAY, per invariant 11).
  - IF (and only if) a stray feature gate on X11 is found: remove it so X11 is
    target-gated only. Otherwise: NO edit. Document the verification in the commit msg.

Task 5: DOCUMENT the hidapi link nuance (part d)
  - IN Cargo.toml, add a comment IMMEDIATELY ABOVE the `hidapi = "2.6"` line:
        # hidapi link nuance (ARCHITECTURE invariant 12 / PACKAGING.md §2):
        # Arch ships libhidapi-hidraw SEPARATE from libhidapi-libusb, so the Arch
        # PKGBUILD (packaging/linux/arch/PKGBUILD) passes
        #   RUSTFLAGS="-C link-arg=-lhidapi-hidraw"
        # explicitly — usage/usage_page device matching REQUIRES the hidraw backend.
        # Debian/Ubuntu/Fedora (+ modern Nixpkgs) ship a UNIFIED hidapi (>=0.14)
        # that auto-selects hidraw at runtime, so the .deb/.rpm (P1.M7) and Nix
        # builds must NOT pass -lhidapi-hidraw (linking the unified lib is correct).
        # Do NOT add a global -lhidapi-hidraw rustflag in .cargo/config.toml — it
        # would break unified-hidapi distros. Keep the flag per-channel.
        hidapi = "2.6"
  - IN .cargo/config.toml, append a comment-only stanza (NO rustflag):
        # NOTE (hidapi link nuance — ARCHITECTURE invariant 12 / PACKAGING.md §2):
        # There is INTENTIONALLY no global `[target.'cfg(target_os = "linux")']
        # rustflags = [… "-C" "link-arg=-lhidapi-hidraw"]` here. Arch needs that
        # flag (separate libhidapi-hidraw) and gets it via env RUSTFLAGS in its
        # PKGBUILD; but Debian/Ubuntu/Fedora (+ modern Nixpkgs) ship a UNIFIED
        # hidapi (>=0.14) where the flag either fails (`cannot find
        # -lhidapi-hidraw`) or mis-links the backend, breaking usage/usage_page
        # matching. The flag stays PER-CHANNEL (Arch PKGBUILD env, Nix hidrawFlag).
        # The only rustflag in this file is the Windows MSVC +crt-static below.
  - DO NOT add any rustflag. The Windows +crt-static stanza stays as-is.

Task 6: VERIFY build + feature toggles + tests
  - RUN: cargo build --bin qmkonnect                                  (default; 4 new crates build — GOTCHA-2/3)
  - RUN: cargo build --bin qmkonnect --no-default-features            (minimal; 4 crates ABSENT — proves optional)
  - RUN: cargo build --bin qmkonnect --features wayland               (toggles smithay-client-toolkit + wayland-client on)
  - RUN: cargo build --bin qmkonnect --features gnome                 (toggles zbus on)
  - RUN: cargo build --bin qmkonnect --features atspi                 (toggles atspi on)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1               (no regression; probe stubs now live + graceful)
  - RUN: grep -rn 'feature.*x11\|x11.*feature' src/                   (empty — X11 ungated)
  - RUN: git diff --stat                                              (ONLY Cargo.toml + .cargo/config.toml)
  - IF `cargo build` (default) fails on a wayland-sys link step: install libwayland
    dev package (GOTCHA-3); this is an environment prerequisite, not a code defect.
  - IF `cargo build` fails with a zbus two-major conflict: atspi 0.30 moved zbus
    majors — align the `zbus` version to atspi's (GOTCHA-5).
```

### Implementation Patterns & Key Details

```toml
# === Cargo.toml [features] — the EXACT target (transcribe from spec/PACKAGING.md §2) ===
[features]
default = ["wayland", "gnome", "atspi", "hyprland", "macos", "linux-tray"]
# Linux window-monitor backends (runtime-selected by select_linux_backend,
# PLATFORMS.md §6). All default-on so a single binary works everywhere;
# turn a backend off to shrink the binary / drop a dep (spec/PACKAGING.md §2).
wayland    = ["dep:smithay-client-toolkit", "dep:wayland-client"]
gnome      = ["dep:zbus"]
atspi      = ["dep:atspi"]
hyprland   = ["dep:hyprland"]
linux-tray = ["dep:ksni", "dep:gtk"]
macos      = ["dep:objc", "dep:core-foundation", "dep:core-graphics", "dep:dispatch"]

# === Cargo.toml [target.'cfg(target_os = "linux")'.dependencies] — ADD these 4 ===
# (keep the existing hyprland/libxdo/tempfile/libc/ksni/gtk lines unchanged)
smithay-client-toolkit = { version = "0.20", optional = true }
wayland-client         = { version = "0.31", optional = true }
zbus                   = { version = "5",   optional = true }
atspi                  = { version = "0.30", optional = true }

# === Cargo.toml [dependencies] — comment above hidapi (part d) ===
# hidapi link nuance (ARCHITECTURE invariant 12 / PACKAGING.md §2): see full text
# in Task 5 above. Short form: Arch PKGBUILD passes -lhidapi-hidraw via env
# RUSTFLAGS; .deb/.rpm/Nix (unified hidapi >=0.14) must NOT. Never globalize it.
hidapi = "2.6"

# === .cargo/config.toml — append comment-only (part d); NO new rustflag ===
# (full text in Task 5). The Windows +crt-static stanza is the ONLY rustflag here.
```

### Integration Points

```yaml
BUILD:
  - default features now compile 4 new optional crates on Linux (GOTCHA-2).
  - --no-default-features yields the minimal trayless X11-only build (unchanged
    contract; PACKAGING.md §2).
  - the build host needs libwayland (dev) for the wayland feature (GOTCHA-3).

PACKAGING (downstream, NOT edited by this task — documentation only):
  - Arch PKGBUILD: KEEPS -lhidapi-hidraw (packaging/linux/arch/PKGBUILD:25) — correct.
  - Nix flake: KEEPS hidrawFlag with documented fallback (flake.nix) — correct.
  - .deb (cargo-deb, P1.M7): must build WITHOUT -lhidapi-hidraw (documented in
    Cargo.toml + .cargo/config.toml comments for the P1.M7 agent).
  - .rpm (cargo-generate-rpm, P1.M7): must build WITHOUT -lhidapi-hidraw (same).

CODE (downstream, NOT edited by this task):
  - select_linux_backend (P2.M1.T1.S1) already has #[cfg(feature="wayland"/"gnome"/
    "atspi")] probe stubs; they become live (graceful Err) under default features.
  - wayland_ft.rs (P2.M2.T1) / gnome.rs (P2.M3.T2) / atspi.rs (P2.M4.T1) will
    `use` these crates behind their features — this task's deps are their targets.

CONFIG: none (LinuxConfig is P2.M1.T2.S1, parallel, owns src/core/mod.rs).
ROUTES: none.
DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
# TOML validity — cargo parses Cargo.toml + .cargo/config.toml on every build.
cargo build --bin qmkonnect --no-default-features   # parses Cargo.toml; minimal graph
# Expected: compiles clean. If cargo errors on Cargo.toml (e.g. duplicate key,
# malformed dep: line), READ the error and fix before proceeding.

# Confirm the features + deps are present:
grep -nE '^default|^wayland|^gnome|^atspi|^hyprland|^linux-tray|^macos' Cargo.toml
# Expected: default=[…wayland…gnome…atspi…] + the 6 feature lines.
grep -nE 'smithay-client-toolkit|wayland-client|^zbus|^atspi' Cargo.toml
# Expected: the 4 new optional deps under [target.'cfg(target_os = "linux")'].
grep -n 'hidapi-hidraw' Cargo.toml .cargo/config.toml
# Expected: the documentation COMMENT text (NOT a rustflag) in both files.
```

### Level 2: Feature-toggle matrix (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect                                  # default: all 4 new crates present
cargo build --bin qmkonnect --no-default-features            # minimal: all 4 ABSENT
cargo build --bin qmkonnect --features wayland               # smithay-client-toolkit + wayland-client
cargo build --bin qmkonnect --features gnome                 # zbus
cargo build --bin qmkonnect --features atspi                 # atspi (+ its zbus)
# Expected: each compiles. --no-default-features proves the deps are genuinely
# optional + dep:-gated (cargo tree shows them absent). If the default build fails
# on a wayland-sys link, install libwayland-dev (GOTCHA-3) — environment, not code.

# Optional: confirm the dep graph toggles as expected:
cargo tree --bin qmkonnect -e normal | grep -cE 'smithay-client-toolkit|wayland-client|zbus|atspi'  # default: >0
cargo tree --bin qmkonnect -e normal --no-default-features | grep -cE 'smithay-client-toolkit|wayland-client|zbus|atspi'  # 0
```

### Level 3: Regression (System Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Crate-wide tests MUST stay single-threaded (shared debouncer state in notifier.rs).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL bin tests green. The select_linux_backend probe stubs are now live
# under default features and return graceful Err("…not yet implemented (P2.M2/…)") —
# that is BY DESIGN (the transitional state until the backends land). No test should
# regress; if one does, it likely hard-coded the OLD feature set.

# Confirm the change surface:
git status --short
# Expected:
#   modified:   Cargo.toml
#   modified:   .cargo/config.toml
git diff --stat
# Expected: ONLY Cargo.toml + .cargo/config.toml (no src/ unless a stray X11 gate
# was removed — none exists today).
```

### Level 4: Parity cross-check (optional, high-confidence)

```bash
# Diff the implemented [features] block against the canonical spec.
diff <(sed -n '/^\[features\]/,/^\[/p' Cargo.toml | grep -v '^\[') \
     <(sed -n '/^default   = /,/^macos/p' spec/PACKAGING.md | sed 's/^#.*//')
# Expected: the feature NAMES + dep: wiring match PACKAGING.md §2 (the comment
# text may differ; the TOML structure must match). The `default` line must list
# all six features.

# Confirm the hidapi nuance is documented in BOTH files:
grep -A2 'hidapi link nuance\|hidapi-hidraw' Cargo.toml .cargo/config.toml

# Confirm .cargo/config.toml has NO new rustflag (only Windows +crt-static):
grep -c 'link-arg=-lhidapi-hidraw' .cargo/config.toml   # expect: 0 (comment mentions it; no rustflag)
grep -c 'rustflags' .cargo/config.toml                   # expect: 1 (the Windows stanza)
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` (default) compiles — 4 new crates build (GOTCHA-2/3).
- [ ] `cargo build --bin qmkonnect --no-default-features` compiles — 4 crates absent (genuinely optional).
- [ ] `cargo build --bin qmkonnect --features wayland` / `gnome` / `atspi` each toggle their deps on.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes (no regression).
- [ ] `git diff --stat` shows ONLY `Cargo.toml` + `.cargo/config.toml`.

### Feature Validation (parity with spec/PACKAGING.md §2)
- [ ] `default` includes `wayland`, `gnome`, `atspi`, `hyprland`, `macos`, `linux-tray`.
- [ ] `wayland = ["dep:smithay-client-toolkit", "dep:wayland-client"]`, `gnome = ["dep:zbus"]`, `atspi = ["dep:atspi"]` (dep: syntax — GOTCHA-7).
- [ ] The 4 crates are `optional = true` under `[target.'cfg(target_os = "linux")'.dependencies]`.
- [ ] `grep -rn 'feature.*x11\|x11.*feature' src/` returns nothing (X11 ungated — GOTCHA-1).
- [ ] hidapi link nuance documented in `Cargo.toml` (near `hidapi`) AND `.cargo/config.toml` (comment, no rustflag — GOTCHA-4).
- [ ] `.cargo/config.toml` has NO `-lhidapi-hidraw` rustflag (only Windows `+crt-static`).

### Code Quality Validation
- [ ] `dep:` syntax used for the 3 new features (matches existing hyprland/linux-tray/macos).
- [ ] Crate versions MSRV-compatible (SCTK 0.20 MSRV 1.86 < 1.88; GOTCHA-6); zbus 5 + atspi 0.30 share zbus 5 (GOTCHA-5).
- [ ] Comments in Cargo.toml kept in sync with spec/PACKAGING.md §2 + ARCHITECTURE invariant 12.
- [ ] No source-code edits beyond X11 verification (no stray gate found ⇒ none).
- [ ] Scope respected: no backend modules, no `select_linux_backend` / `LinuxConfig` edits (parallel siblings own those).

### Documentation & Deployment
- [ ] Mode A: the feature table in spec/PACKAGING.md §2 is the reference; Cargo.toml comments cite it.
- [ ] The hidapi comment gives the per-distro rule (Arch WITH; Debian/Fedora/Nix WITHOUT) so P1.M7 inherits it.

---

## Anti-Patterns to Avoid

- ❌ Do NOT leave wayland/gnome/atspi OUT of `default` to keep the build light. PACKAGING.md
      §2 mandates all-three-in-default ("a single binary works everywhere"). That they
      compile while unused (until P2.M2–P2.M4) is the intended transitional cost (GOTCHA-2).
- ❌ Do NOT add a global `-lhidapi-hidraw` rustflag to `.cargo/config.toml`. It breaks
      unified-hidapi distros (.deb/.rpm/Nix). The flag is PER-CHANNEL (Arch PKGBUILD env,
      Nix hidrawFlag). Document the absence with a comment, not a rustflag (GOTCHA-4).
- ❌ Do NOT make X11 a Cargo feature. It is unconditional on Linux by design; the
      Wayland guard is RUNTIME (`$WAYLAND_DISPLAY` unset), not compile-time
      (ARCHITECTURE invariant 11). Part (c) is a verification, not an edit (GOTCHA-1).
- ❌ Do NOT use bare crate names in `[features]` (`wayland = ["smithay-client-toolkit"]`).
      Use `dep:` (`["dep:smithay-client-toolkit"]`) — matches existing features and
      avoids implicit same-named features (GOTCHA-7).
- ❌ Do NOT edit `select_linux_backend` (src/platforms/linux.rs) or `LinuxConfig`
      (src/core/mod.rs). Those are the parallel siblings P2.M1.T1.S1 / P2.M1.T2.S1.
      The probe stubs already reference these features and become live automatically.
- ❌ Do NOT create `wayland_ft.rs` / `gnome.rs` / `atspi.rs`. Those are P2.M2 / P2.M3 / P2.M4.
- ❌ Do NOT guess crate versions. The 4 versions are decided (notes §5): SCTK 0.20,
      wayland-client 0.31, zbus 5, atspi 0.30 — MSRV-compatible and mutually so.
- ❌ Do NOT bump the project MSRV. SCTK 0.20 MSRV 1.86 is below the 1.88 floor (GOTCHA-6).
- ❌ Do NOT change the Arch PKGBUILD or Nix hidrawFlag — they are CORRECT as-is (the
      flag IS needed there). This task only DOCUMENTS the nuance for the future
      .deb/.rpm builds.
- ❌ Do NOT skip the baseline build (Task 1). Establish green-before so any breakage
      is attributable.
- ❌ Do NOT run tests multi-threaded — the crate shares debouncer state
      (`cargo test --bin qmkonnect -- --test-threads=1`).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, or any `plan/` file
      other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

This is a small, well-bounded build-configuration task whose target state is quoted
VERBATIM from the canonical spec (`spec/PACKAGING.md` §2) in `research/notes.md` §1,
whose CURRENT state (the BEFORE) is recorded in §2 so the agent knows exactly what
to change, and whose two non-obvious pieces are fully resolved: (1) part (c) is
shown to be ALREADY done in the tree with the three verification sites + grep command
(GOTCHA-1 — a no-op verification, not a hunt), and (2) the hidapi nuance is pinned to
exact file:line evidence (§4) with the exact comment text to write and an explicit
"do NOT globalize the rustflag" guard (GOTCHA-4). The four crate versions + MSRV
compatibility are decided (§5: SCTK 0.20 MSRV 1.86 < project 1.88; zbus 5 + atspi 0.30
share zbus 5), removing all guesswork. The two load-bearing build-time gotchas — that
adding to `default` compiles the new crates even while unused (GOTCHA-2), and that
`libwayland` may need installing on the build host (GOTCHA-3) — are explicit, with
the `--no-default-features` escape hatch for verification. The scope boundary with
the two parallel siblings (P2.M1.T1.S1 owns `select_linux_backend`; P2.M1.T2.S1 owns
`LinuxConfig`) is explicit, so there's no overlap risk. The 1-point reservation is
for the (unlikely) event the build host lacks `libwayland` and the agent doesn't
recognize the wayland-sys link failure as an environment prerequisite (GOTCHA-3) —
recoverable by installing the dev package or verifying with `--no-default-features`.
No new deps beyond the 4 specified; no `unsafe`; no source changes expected.