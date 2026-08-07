# PRP — P1.M7.T1.S1: `.deb` package via cargo-deb (`[package.metadata.deb]` + maintainer scripts + long-description)

---

## Goal

**Feature Goal**: Enable `cargo-deb` to produce a correct, installable Debian
package for QMKonnect by adding a `[package.metadata.deb]` block to `Cargo.toml`
and the supporting maintainer scripts + long-description, so the resulting
`target/debian/qmkonnect_<ver>_amd64.deb` ships the full Linux artifact set
(binary, hid-id udev helper, static udev rule, systemd user-service template, XDG
autostart `.desktop`, README) to the exact FHS paths mandated by
`spec/PACKAGING.md` §4, and runs the install/uninstall hooks that mirror the Arch
`qmkonnect.install`.

**Deliverable**:
1. A `[package.metadata.deb]` table appended to `Cargo.toml` (verbatim per
   `spec/PACKAGING.md` §4.3).
2. `packaging/debian/long-description.txt` — the extended Debian package
   description.
3. `packaging/debian/postinst` — mirrors Arch `post_install` (Debian/POSIX-sh).
4. `packaging/debian/prerm` — documented no-op.
5. `packaging/debian/postrm` — mirrors Arch `post_remove` (Debian/POSIX-sh).
6. `packaging/debian/README.md` — Mode-A doc: how to build + install the `.deb`.

**Success Definition**: `cargo install cargo-deb && cargo deb` (run on a
Debian/Ubuntu host, or ubuntu-22.04 in CI) emits
`target/debian/qmkonnect_0.2.8_amd64.deb`; `ar` inspection shows all 6 assets at
the correct target paths, correct control fields (`Depends`, `Maintainer`,
`License`, `Section`), and the three maintainer scripts present in the control
archive; the build used **no** `-lhidapi-hidraw` link flag; the maintainer scripts
are POSIX-`sh` (dash-safe) and the `input`-group enablement is idempotent.

## Why

- **F15 (community package-manager distribution)** calls for native `.deb`/`.rpm`
  alongside AUR/Homebrew/Scoop/Winget/Nix/mise/asdf (`PACKAGING.md` §6, F15 row).
  Ubuntu/Debian/Mint users currently have only the generic tarball; a `.deb`
  gives them `sudo apt install ./…` one-liners and proper dependency resolution.
- **The artifact set is already shared**: every Linux channel installs the same
  binary + helper + static rule + service template (+ new `.desktop`) to the same
  FHS paths (`PACKAGING.md` §4 artifact table). The `.deb` just wraps that set in
  dpkg metadata + the install/uninstall lifecycle the Arch package already proves
  out via `qmkonnect.install`.
- **Scope boundary**: this task produces ONLY the `.deb` build recipe + scripts +
  doc. The `.rpm` sibling is **P1.M7.T2**; wiring the `.deb` into the CI release
  pipeline (ubuntu-22.04 build job, release renaming) is **P1.M7.T1.S2**. The XDG
  `.desktop` file itself is **P2.M6.T1.S1** — consumed here, not authored here
  (see the hard-prerequisite handling below).

## What

User-/maintainer-visible behavior:
- `cargo deb` reads the new `[package.metadata.deb]` block and packages the
  release binaries + the 4 data assets into `target/debian/qmkonnect_0.2.8_amd64.deb`.
- On `dpkg -i`, `postinst` instantiates the user service, reloads udev, globally
  enables the service, ensures the `input` group exists, and prints zero-config
  next-steps — exactly like the Arch `post_install`.
- On removal, `postrm` disables globally, stops/disables per-user instances,
  removes the instantiated service + any user-generated
  `/etc/udev/rules.d/99-qmkonnect.rules`, and reloads udev.
- `packaging/debian/README.md` documents `cargo deb` + `sudo dpkg -i` /
  `sudo apt install ./…`.

### Success Criteria

- [ ] `[package.metadata.deb]` block present in `Cargo.toml`, byte-for-byte the
      §4.3 recipe (6-asset array, `depends`, `maintainer-scripts`, etc.).
- [ ] `packaging/debian/{long-description.txt,postinst,prerm,postrm}` all created.
- [ ] Maintainer scripts start with `#!/bin/sh` and contain **no bashisms**
      (notably `&>` translated to `>/dev/null 2>&1`).
- [ ] `packaging/debian/README.md` created (Mode-A build+install doc).
- [ ] `cargo deb` produces `target/debian/qmkonnect_0.2.8_amd64.deb` whose
      `data.tar` contains the 6 assets at the §4 target paths and whose control
      archive contains `postinst`/`prerm`/`postrm`.
- [ ] Build uses **no** `-lhidapi-hidraw` (Debian unified hidapi — §2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` still green (no `.rs`
      touched; regression guard).

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?" — **YES.** The exact `[package.metadata.deb]`
> recipe is pinned in `spec/PACKAGING.md` §4.3; the maintainer-script logic is
> pinned in the Arch `qmkonnect.install` (and translated to POSIX-sh below); the
> `.desktop` content is pinned in §4.7; every asset path is enumerated and
> verified-present (except the `.desktop` prerequisite, handled explicitly).

### Documentation & References

```yaml
# MUST READ — the authoritative recipe (copy the TOML block verbatim)
- file: spec/PACKAGING.md
  section: "§4.3 .deb via cargo-deb (packaging/debian/) — NEW"
  why: Contains the EXACT [package.metadata.deb] block, the depends line, the
       maintainer-script behavior contract, the no-hidraw-flag build note, and
       the output-path / CI-rename spec.
  critical: |
    Build WITHOUT -lhidapi-hidraw (Debian unified hidapi auto-selects hidraw — §2).
    depends = "libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd".
    Build on ubuntu-22.04 for glibc 2.35 compat (that is the CI job, P1.M7.T1.S2).

# MUST READ — the Linux hidapi link nuance (the single most important build fact)
- file: spec/PACKAGING.md
  section: "§2 Features & Binaries → 'Linux hidapi link nuance (must-preserve)'"
  why: Explains WHY the .deb must NOT pass -lhidapi-hidraw (Debian/Ubuntu ship a
       unified hidapi >=0.14 that folds both backends into libhidapi.so and
       auto-selects hidraw), while the Arch PKGBUILD MUST (Arch ships hidraw/libusb
       split). Getting this backwards breaks usage/usage_page matching at runtime.
  critical: Same note as the Nix flake's hidrawFlag caveat.

# MUST READ — the FHS artifact table (target paths the assets must land at)
- file: spec/PACKAGING.md
  section: "§4 Linux Packaging (artifact table)"
  why: The 7-row table pins the exact install path for every file. The assets
       array target dirs must produce these paths.
  pattern: |
    app binary           -> /usr/bin/qmkonnect
    udev helper          -> /usr/lib/udev/qmkonnect-hid-id
    static udev rule     -> /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules
    service (template)   -> /usr/lib/systemd/user/qmkonnect.service.template
    service (instantiated)-> /usr/lib/systemd/user/qmkonnect.service  (postinst writes)
    XDG autostart        -> /etc/xdg/autostart/qmkonnect.desktop
    docs                 -> /usr/share/doc/qmkonnect/

# MUST READ — the maintainer-script logic source of truth (translate to POSIX sh)
- file: packaging/linux/arch/qmkonnect.install
  why: post_install/post_upgrade/post_remove ARE the logic the Debian
       postinst/prerm/postrm must mirror (contract INPUT). The Debian variants
       differ only in: #!/bin/sh shebang, POSIX-sh (no &>), addgroup for the
       input group, and Debian maintainer-script calling conventions.
  gotcha: qmkonnect.install uses the bashism `&>/dev/null` (in post_remove's
          systemctl status check) — dash rejects it. Translate to
          `>/dev/null 2>&1` in postrm.

# MUST READ — the service template the postinst instantiates
- file: packaging/linux/systemd/qmkonnect.service.template
  why: postinst copies this verbatim to /usr/lib/systemd/user/qmkonnect.service,
       then `systemctl --global enable`s it. (No VID/PID substitution — the
       service binds to the qmkonnect_device symlink the static udev rule makes.)

# MUST READ — the static udev rule the package ships
- file: packaging/linux/udev/69-qmkonnect-rawhid.rules
  why: Shipped verbatim as an asset; postinst reloads it (udevadm control
       --reload-rules && udevadm trigger).

# HARD PREREQUISITE — the XDG autostart .desktop (owned by P2.M6.T1.S1)
- file: packaging/linux/xdg/qmkonnect.desktop
  why: The §4.3 assets array references it -> target /etc/xdg/autostart/. cargo
       deb FAILS if the source file is absent. P2.M6.T1.S1 owns this file, but
       its content is 100% pinned by spec/PACKAGING.md §4.7 (see Task 1).
  critical: |
    If this file does NOT exist when you build, see Task 1's fallback: create it
    verbatim from the §4.7 content below. The file is byte-identical to what
    P2.M6.T1.S1 produces, so creating it as an unblock is idempotent (no conflict).

# REFERENCE — sibling packaging READMEs (mirror their style for packaging/debian/README.md)
- file: packaging/linux/aur/README.md
  why: Style template: H1 title, "What this is", fenced "Install" commands,
       relative cross-links to spec/PACKAGING.md + docs/installation.md.
- file: packaging/nix/README.md
  why: Same style; shows how to document a from-source build + system-dep notes.

# EXTERNAL — cargo-deb config semantics
- url: https://github.com/kornelski/cargo-deb#configuration
  why: Canonical [package.metadata.deb] field reference (maintainer-scripts dir
       naming, assets [source,target,mode], license-file 2-elem array,
       extended-description-file).
  critical: |
    - maintainer-scripts dir holds BARE-named postinst/prerm/postrm (no .sh ext).
    - license-file = ["LICENSE","N"] : N = lines to skip at top; "0" = whole file.
    - See "Known Gotchas" for the #DEBHELPER#/systemd interaction.
```

### Current Codebase Tree (relevant slice)

```bash
Cargo.toml                                   # [package] name=qmkonnect v0.2.8; NO [package.metadata.deb] yet
.cargo/config.toml                           # windows-msvc +crt-static only (Linux untouched — good, no hidraw flag)
packaging/
├── linux/
│   ├── udev/69-qmkonnect-rawhid.rules       # ASSET ✓ (ships verbatim)
│   ├── systemd/qmkonnect.service.template   # ASSET ✓ (postinst instantiates)
│   ├── xdg/qmkonnect.desktop                # ❌ ABSENT — P2.M6.T1.S1 (see Task 1)
│   └── arch/qmkonnect.install               # LOGIC SOURCE for the maintainer scripts
├── debian/                                  # ❌ DOES NOT EXIST — this task creates it
└── (aur/, nix/, homebrew/, scoop/, winget/, asdf/, windows/, macos/ — existing channels)
README.md  LICENSE                           # ASSETS ✓ (README -> doc; LICENSE -> copyright)
target/release/{qmkonnect,qmkonnect-hid-id}  # ASSETS ✓ (built by cargo build --release)
spec/PACKAGING.md                            # READ-ONLY source of truth (§2, §4, §4.3, §4.7)
```

### Desired Codebase Tree (files this task creates/edits)

```bash
Cargo.toml                                   # EDIT: append [package.metadata.deb] table at EOF
packaging/debian/                            # NEW dir
├── long-description.txt                     # NEW — extended Debian description
├── postinst                                 # NEW — POSIX-sh, mirrors post_install
├── prerm                                    # NEW — POSIX-sh no-op
├── postrm                                   # NEW — POSIX-sh, mirrors post_remove
└── README.md                                # NEW — Mode-A build+install doc
# (packaging/linux/xdg/qmkonnect.desktop is a PRECONDITION, not a deliverable
#  of this task — created by P2.M6.T1.S1 or the Task-1 §4.7 fallback.)
```

### Known Gotchas of our codebase & library quirks

```bash
# CRITICAL (build correctness): The .deb build must NOT pass -lhidapi-hidraw.
# Debian/Ubuntu ship a UNIFIED hidapi (>=0.14) in libhidapi.so that auto-selects
# the hidraw backend at runtime; usage/usage_page matching works without the flag.
# The Arch PKGBUILD sets RUSTFLAGS="-C link-arg=-lhidapi-hidraw" ONLY because Arch
# ships the hidraw/libusb backends as SEPARATE libs. (spec/PACKAGING.md §2.)
# => `cargo deb` (which runs a plain `cargo build --release`) is correct as-is;
#    do NOT add a RUSTFLAGS hidraw flag to the .deb path anywhere.

# CRITICAL (cargo-deb systemd auto-detection): cargo-deb detects unit files whose
# target path ends in .service/.socket/.timer and AUTO-INJECTS debhelper fragments
# into maintainer scripts, REQUIRING a `#DEBHELPER#` insertion token. We ship
# qmkonnect.service.TEMPLATE (not .service) -> the filename does NOT match ->
# cargo-deb does NOT detect a unit -> NO fragments -> NO #DEBHELPER# token.
# DO NOT add #DEBHELPER# to the scripts (a literal leftover would be a bug). Our
# manual `systemctl --global enable` in postinst is the sole enablement path.
# This mirrors the Arch package (ships .service.template; post_install instantiates).

# CRITICAL (POSIX sh): Debian maintainer scripts run under /bin/sh = dash, NOT bash.
# The Arch qmkonnect.install uses the bashism `&>/dev/null` (stdout+stderr) which
# dash REJECTS -> translate to `>/dev/null 2>&1` in postrm. Everything else in
# qmkonnect.install (heredoc <<'EOF', [ -d ], for-in-/home/*, id -u, su -c,
# basename, 2>/dev/null) is already POSIX-safe.

# CRITICAL (input group idempotency): Debian ships the `input` group by default,
# so a bare `addgroup --system input` ERRORS on most systems. Guard it:
#   getent group input >/dev/null 2>&1 || addgroup --system input
# (`addgroup` is Debian-specific, from the adduser package; correct for a .deb.)

# GOTCHA (host build env): This dev box is ARCH (no dpkg-deb). A local `cargo deb`
# may try to relink the hidapi crate against Arch's SPLIT libs and FAIL without
# the hidraw flag — an Arch-only artifact, NOT a defect of the .deb recipe (which
# targets Debian). For local config/structure validation either (a) rely on the
# already-built target/release/* (cargo skips relink if fresh), or (b) set
# RUSTFLAGS="-C link-arg=-lhidapi-hidraw" for the LOCAL build only. The
# AUTHORITATIVE build is ubuntu-22.04 CI (P1.M7.T1.S2).

# GOTCHA (cargo-deb not installed): run `cargo install cargo-deb` first. It builds
# the .deb on any Unix host (constructs the ar archive itself); it does NOT need
# dpkg installed on the build host.

# GOTCHA (assets must exist at build time): cargo-deb fails if any asset source
# path is missing. All resolve EXCEPT packaging/linux/xdg/qmkonnect.desktop
# (P2.M6.T1.S1) — see Task 1.
```

## Implementation Blueprint

### Data models and structure

_None._ This is packaging metadata + shell scripts + a doc. No Rust types,
schemas, or runtime models change. The only edit to `Cargo.toml` is an appended
metadata table (does not affect the build fingerprint for code).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: SATISFY the packaging/linux/xdg/qmkonnect.desktop prerequisite
  - CHECK: test -f packaging/linux/xdg/qmkonnect.desktop
  - IF ABSENT (P2.M6.T1.S1 not yet landed): create it VERBATIM from
    spec/PACKAGING.md §4.7 (content pinned below). This unblocks `cargo deb`.
    The file is byte-identical to P2.M6.T1.S1's output, so this is idempotent and
    does NOT conflict — but NOTE in the commit that the .desktop itself is owned
    by P2.M6.T1.S1; this task only consumes it as a build asset.
  - IF PRESENT: verify its content matches §4.7; proceed.
  - EXACT §4.7 content to use when creating the fallback:
      ------------------------------------------------------------
      [Desktop Entry]
      Type=Application
      Name=QMKonnect
      Comment=Send the foreground window to your QMK keyboard
      Exec=qmkonnect
      Icon=input-keyboard
      Terminal=false
      X-GNOME-Autostart-enabled=true
      Categories=Utility;
      # Not shown in application menus (autostart-only):
      NoDisplay=true
      ------------------------------------------------------------
  - DEPENDENCIES: none (this is the unblock step).

Task 2: EDIT Cargo.toml — append the [package.metadata.deb] table
  - APPEND at END of file (after the [profile.release] block) as a NEW top-level
    table — do NOT nest it inside any existing table.
  - USE EXACTLY this block (from spec/PACKAGING.md §4.3):
      [package.metadata.deb]
      name = "qmkonnect"
      maintainer = "Mulletware <noreply@mulletware>"
      copyright = "2025, Mulletware"
      license-file = ["LICENSE", "0"]
      extended-description-file = "packaging/debian/long-description.txt"
      depends = "libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd"
      section = "utils"
      priority = "optional"
      assets = [
        ["target/release/qmkonnect",                   "usr/bin/",                         "755"],
        ["target/release/qmkonnect-hid-id",            "usr/lib/udev/",                    "755"],
        ["packaging/linux/udev/69-qmkonnect-rawhid.rules", "usr/lib/udev/rules.d/",        "644"],
        ["packaging/linux/systemd/qmkonnect.service.template", "usr/lib/systemd/user/",   "644"],
        ["packaging/linux/xdg/qmkonnect.desktop",      "etc/xdg/autostart/",               "644"],
        ["README.md",                                  "usr/share/doc/qmkonnect/",         "644"],
      ]
      maintainer-scripts = "packaging/debian/"
  - PRESERVE: the existing [package] fields (version 0.2.8, description, license
    "MIT"), all [dependencies], [features], [[bin]], [profile.release]. This is a
    pure append; touch NOTHING else.
  - NOTE on license-file = ["LICENSE","0"]: "0" = skip 0 lines -> whole LICENSE
    becomes /usr/share/doc/qmkonnect/copyright. Coexists with package-level
    license="MIT" (fills the control License: field). Do not "fix" the 0.

Task 3: CREATE packaging/debian/long-description.txt
  - PURPOSE: the EXTENDED description in the Debian control file (NOT the
    synopsis — synopsis = first line of Cargo.toml description).
  - CONTENT: 1-3 short paragraphs. Base on README.md's opening + the headline
    capability (auto-discovers QMK boards by the Raw HID usage page 0xFF60/0x61;
    ships a static udev rule + systemd user service + XDG autostart so it Just
    Works at login with zero config). Mention it works on X11/Wayland/Hyprland/
    GNOME/etc. Keep under ~500 chars. Plain text, no markdown.
  - EXAMPLE wording (adapt, keep factual):
      QMKonnect detects the foreground window on your desktop and tells your
      QMK keyboard which app is active, so the board can switch layers or fire
      callbacks automatically — no reflash needed for host-side rule changes.
      .
      Default QMK keyboards are auto-discovered by the standard Raw HID usage
      page (0xFF60 / 0x61); a static udev rule grants permissions and a systemd
      user service (plus an XDG autostart fallback) start the daemon at login
      with zero configuration. Works across X11 and Wayland compositors
      (Hyprland, Sway, KDE, GNOME, COSMIC, …).
  - NOTE: a line containing only "." is the Debian control-file paragraph break
    convention; cargo-deb passes the file through, so include the " ." separators.

Task 4: CREATE packaging/debian/postinst  (mirrors Arch post_install, POSIX sh)
  - SHEBANG: #!/bin/sh  then  set -e
  - LOGIC (port from packaging/linux/arch/qmkonnect.install post_install):
      1. Instantiate the user service from its template:
           if [ -f /usr/lib/systemd/user/qmkonnect.service.template ]; then
             install -m644 \
               /usr/lib/systemd/user/qmkonnect.service.template \
               /usr/lib/systemd/user/qmkonnect.service
           fi
      2. Reload + trigger udev:
           udevadm control --reload-rules
           udevadm trigger
      3. Ensure the input group exists (idempotent — Debian ships it by default):
           getent group input >/dev/null 2>&1 || addgroup --system input
      4. Globally enable the user service (best-effort):
           systemctl --global enable qmkonnect.service >/dev/null 2>&1 || true
      5. Print the zero-config next-steps heredoc (copy the cat <<'EOF' ... EOF
         block verbatim from qmkonnect.install post_install).
  - POSIX CHECK: no `&>`. Use `>/dev/null 2>&1` everywhere.
  - DEPENDENCIES: Task 1 (.desktop) + Task 2 (metadata) for a successful build,
    but the script content is independent of those.

Task 5: CREATE packaging/debian/prerm  (documented no-op, POSIX sh)
  - CONTENT (exactly):
      #!/bin/sh
      # prerm: no-op. The running per-user service is left alone until reboot or
      # an explicit `systemctl --user stop`; real teardown happens in postrm.
      # (Mirrors the Arch package, which has no pre_remove hook.)
      set -e
      exit 0

Task 6: CREATE packaging/debian/postrm  (mirrors Arch post_remove, POSIX sh)
  - SHEBANG: #!/bin/sh  then  set -e
  - LOGIC (port from qmkonnect.install post_remove, TRANSLATING the bashism):
      1. Disable globally (best-effort):
           systemctl --global disable qmkonnect.service >/dev/null 2>&1 || true
      2. Stop + disable per-user instances for each /home/* user:
           for user_home in /home/*; do
             [ -d "$user_home" ] || continue
             username=$(basename "$user_home")
             user_id=$(id -u "$username" 2>/dev/null) || continue
             if systemctl --user -M "$username@" status qmkonnect.service >/dev/null 2>&1; then
               echo "Stopping service for $username..."
               su "$username" -c "XDG_RUNTIME_DIR=/run/user/$user_id systemctl --user stop qmkonnect.service"
               su "$username" -c "XDG_RUNTIME_DIR=/run/user/$user_id systemctl --user disable qmkonnect.service"
             fi
           done
         # CRITICAL: the Arch original wrote `&>/dev/null` on the `systemctl
         # ... status` line — dash rejects it. Use `>/dev/null 2>&1` (done above).
      3. Remove generated/user-created files (dpkg already removes package-owned
         files like the static rule + helper + template):
           rm -f /etc/udev/rules.d/99-qmkonnect.rules
           rm -f /usr/lib/systemd/user/qmkonnect.service
      4. Reload + trigger udev:
           udevadm control --reload-rules
           udevadm trigger
      5. echo "QMKonnect has been successfully removed."
  - NOTE: postrm runs for both `remove` and `purge`; the cleanup above is correct
    for both (the instantiated .service and the user-generated 99- rule are never
    package-owned, so they must go on removal).

Task 7: CREATE packaging/debian/README.md  (Mode-A build + install doc)
  - STYLE: mirror packaging/linux/aur/README.md + packaging/nix/README.md (H1,
    "What this is", fenced "Build"/"Install" commands, relative cross-links).
  - RELATIVE LINKS from packaging/debian/ (3 levels up to repo root):
      spec:     ../../../spec/PACKAGING.md     (cite §4.3 + §4)
      install:  ../../../docs/installation.md
      readme:   ../../../README.md
  - CONTENT must cover:
      * What it is: the native Debian/Ubuntu/Mint package built with cargo-deb
        from a [package.metadata.deb] block in Cargo.toml (spec §4.3).
      * Build (on Debian/Ubuntu — unified hidapi, NO -lhidapi-hidraw):
          cargo install cargo-deb
          cargo build --release
          cargo deb
        Output: target/debian/qmkonnect_0.2.8_amd64.deb
        (CI builds on ubuntu-22.04 for glibc 2.35 compat; the release artifact is
        renamed qmkonnect-<ver>-linux-amd64.deb — owned by P1.M7.T1.S2.)
      * Install:
          sudo dpkg -i target/debian/qmkonnect_*.deb
          # or, to auto-resolve the Depends:
          sudo apt install ./target/debian/qmkonnect_*.deb
      * Runtime depends: libhidapi-hidraw0, libxdo3, zenity, libnotify-bin,
        systemd. Build-deps (apt): libhidapi-dev libxdo-dev pkg-config.
      * The .deb ships the full Linux artifact set to the §4 FHS paths and runs
        the postinst/postrm hooks (instantiate service, reload udev, systemctl
        --global enable on install; reverse on remove).
      * Cross-link to spec/PACKAGING.md §4.3 (recipe) and §2 (hidapi nuance).

Task 8: BUILD + VALIDATE the .deb
  - INSTALL tool: cargo install cargo-deb
  - BUILD: cargo deb
    (If the local host is Arch and the link fails on the split hidapi lib, set
     RUSTFLAGS="-C link-arg=-lhidapi-hidraw" for the LOCAL validation build ONLY,
     and document that the authoritative build is ubuntu-22.04 CI. Do NOT bake
     the flag into the recipe.)
  - INSPECT without dpkg-deb (works on any host with binutils `ar`):
      DEB=target/debian/qmkonnect_0.2.8_amd64.deb
      ar t "$DEB"                                  # debian-binary, control.tar.*, data.tar.*
      ar p "$DEB" control.tar.gz | tar tz          # lists postinst prerm postrm control
      ar p "$DEB" data.tar.xz  | tar tv            # lists the 6 installed assets + paths
  - ASSERT (see Validation Loop Level 3 for the exact grep checks):
      * data.tar contains: ./usr/bin/qmkonnect, ./usr/lib/udev/qmkonnect-hid-id,
        ./usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules,
        ./usr/lib/systemd/user/qmkonnect.service.template,
        ./etc/xdg/autostart/qmkonnect.desktop, ./usr/share/doc/qmkonnect/README.md,
        ./usr/share/doc/qmkonnect/copyright.
      * control archive contains postinst, prerm, postrm.
      * control Depends = libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd.
  - REGRESSION: cargo test --bin qmkonnect -- --test-threads=1 (no .rs changed).
```

### Implementation Patterns & Key Details

```sh
# The canonical POSIX-sh postinst skeleton (Task 4) — note every redirect is
# `>/dev/null 2>&1`, NEVER `&>`:
#!/bin/sh
set -e

# 1. Instantiate the user service from its template (no VID/PID substitution).
if [ -f /usr/lib/systemd/user/qmkonnect.service.template ]; then
  install -m644 \
    /usr/lib/systemd/user/qmkonnect.service.template \
    /usr/lib/systemd/user/qmkonnect.service
fi

# 2. Load the static usage-page udev rule so default QMK boards Just Work.
udevadm control --reload-rules
udevadm trigger

# 3. Ensure the input group exists (Debian ships it; be idempotent).
getent group input >/dev/null 2>&1 || addgroup --system input

# 4. Enable the user service globally (best-effort).
systemctl --global enable qmkonnect.service >/dev/null 2>&1 || true

# 5. Zero-config next-steps (verbatim from qmkonnect.install).
cat <<'EOF'

QMKonnect installed. Default QMK keyboards need NO configuration: QMKonnect
auto-discovers them by the standard Raw HID usage page (0xFF60 / 0x61), and the
shipped static udev rule already grants permissions (no --reload, no sudo).
...
EOF
```

### Integration Points

```yaml
CARGO.TOML:
  - append: "[package.metadata.deb]" top-level table (Task 2) — exact §4.3 recipe
  - preserve: [package], [dependencies], [features], [[bin]], [profile.release]
  - DO NOT add a [package.metadata.generate-rpm] block — that is P1.M7.T2.S1.

PACKAGING TREE:
  - create dir: packaging/debian/  (long-description.txt, postinst, prerm, postrm, README.md)

PRECONDITION (consumed, not owned here):
  - packaging/linux/xdg/qmkonnect.desktop  (P2.M6.T1.S1; §4.7 fallback in Task 1)

NO CHANGES TO:
  - any .rs, .github/workflows/* (CI is P1.M7.T1.S2), release.toml, .cargo/config.toml
  - spec/PACKAGING.md (read-only source of truth)
  - the Arch PKGBUILDs / AUR / Nix / other channels
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Cargo.toml still parses (the metadata block is well-formed TOML):
cargo metadata --no-deps --format-version 1 >/dev/null && echo "PASS cargo metadata"

# Maintainer scripts are POSIX-sh clean (if shellcheck is available):
shellcheck packaging/debian/postinst packaging/debian/prerm packaging/debian/postrm \
  || echo "(shellcheck not installed — visual review for bashisms)"
# CRITICAL manual check: grep MUST return nothing:
grep -nE '&>' packaging/debian/postinst packaging/debian/prerm packaging/debian/postrm \
  && echo "FAIL: bashism &> present" || echo "PASS: no &> bashism"
# Shebang check:
head -1 packaging/debian/postinst packaging/debian/prerm packaging/debian/postrm
# Expected: #!/bin/sh on all three.

# Expected: cargo metadata exits 0; no &> matches; all shebangs are #!/bin/sh.
```

### Level 2: Asset Resolution (build-input correctness)

```bash
# Every asset source path in the §4.3 array must resolve:
for p in \
  target/release/qmkonnect \
  target/release/qmkonnect-hid-id \
  packaging/linux/udev/69-qmkonnect-rawhid.rules \
  packaging/linux/systemd/qmkonnect.service.template \
  packaging/linux/xdg/qmkonnect.desktop \
  README.md \
  LICENSE ; do
  test -e "$p" && echo "OK      $p" || echo "ABSENT  $p"
done
# Expected: all OK. If packaging/linux/xdg/qmkonnect.desktop is ABSENT, run Task 1
# (create it from §4.7) before proceeding — cargo deb will fail on it otherwise.
```

### Level 3: .deb Build + Structure (the core gate)

```bash
# Install cargo-deb if missing:
cargo deb --version 2>/dev/null || cargo install cargo-deb

# Build the .deb (NO -lhidapi-hidraw; Debian unified hidapi path):
cargo deb
# Arch-host fallback ONLY if the link fails on the split hidapi lib:
#   RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo deb
# (Document this is an Arch-only local-validation workaround; the authoritative
#  build is ubuntu-22.04 CI — P1.M7.T1.S2.)

DEB=target/debian/qmkonnect_0.2.8_amd64.deb
test -f "$DEB" && echo "PASS deb produced: $DEB" || { echo "FAIL no deb"; exit 1; }

# Inspect WITHOUT dpkg-deb (binutils `ar` + tar; works on Arch too):
ar t "$DEB"                                        # must list debian-binary + control.tar.* + data.tar.*

# Data archive: assert the 6 assets + copyright land at the §4 paths:
DATA=$(ar t "$DEB" | grep -E '^data\.tar')
ar p "$DEB" "$DATA" | tar tv | grep -E 'usr/bin/qmkonnect$|usr/lib/udev/qmkonnect-hid-id$|usr/lib/udev/rules.d/69-qmkonnect-rawhid\.rules$|usr/lib/systemd/user/qmkonnect\.service\.template$|etc/xdg/autostart/qmkonnect\.desktop$|usr/share/doc/qmkonnect/README\.md$|usr/share/doc/qmkonnect/copyright$'
# Expected: 7 matching lines (binary, helper, rule, template, .desktop, README, copyright).

# Control archive: assert the 3 maintainer scripts are present:
CTRL=$(ar t "$DEB" | grep -E '^control\.tar')
ar p "$DEB" "$CTRL" | tar tz | grep -E 'postinst$|prerm$|postrm$'
# Expected: ./postinst, ./prerm, ./postrm.

# Control file Depends line:
ar p "$DEB" "$CTRL" | tar xO ./control | grep -E '^Depends:'
# Expected: Depends: libhidapi-hidraw0, libxdo3, zenity, libnotify-bin, systemd
```

### Level 4: Regression (no source touched)

```bash
# No .rs changed — this is a pure regression guard (AGENTS.md mandates single-threaded
# because the debouncer uses shared global state):
cargo test --bin qmkonnect -- --test-threads=1
# Expected: all tests pass (identical to the prior green baseline).
# FALLBACK (only if the host lacks the Linux build deps for a full build):
#   cargo check --bin qmkonnect   # must compile cleanly; record why the full test couldn't run.
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1: `cargo metadata` exits 0; no `&>` bashism; all shebangs `#!/bin/sh`.
- [ ] Level 2: all 6 asset sources + LICENSE resolve (`.desktop` via Task 1 if needed).
- [ ] Level 3: `cargo deb` produces `target/debian/qmkonnect_0.2.8_amd64.deb`;
      `ar`+`tar` inspection shows the 7 data paths + 3 maintainer scripts + correct Depends.
- [ ] Level 4: `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] Build used **no** `-lhidapi-hidraw` in the recipe (any hidraw flag was an
      Arch-only local-validation workaround, clearly documented).

### Feature Validation
- [ ] `[package.metadata.deb]` block present in Cargo.toml, byte-for-byte §4.3.
- [ ] `packaging/debian/{long-description.txt,postinst,prerm,postrm,README.md}` created.
- [ ] postinst logic mirrors Arch `post_install` (instantiate, udev reload,
      `--global enable`, ensure `input` group, next-steps).
- [ ] postrm logic mirrors Arch `post_remove` (global disable, per-user stop/disable,
      remove instantiated service + 99- rule, udev reload).
- [ ] prerm is a documented no-op.
- [ ] `packaging/debian/README.md` documents `cargo deb` + `dpkg -i` / `apt install ./…`.

### Code Quality Validation
- [ ] Maintainer scripts are POSIX-`sh` (dash-safe); no bashisms.
- [ ] `input`-group enablement is idempotent (`getent … || addgroup`).
- [ ] No `#DEBHELPER#` token added (the `.service.template` naming shields us from
      cargo-deb's systemd auto-injection — see Known Gotchas).
- [ ] Cargo.toml edit is a pure append; no existing field disturbed.
- [ ] README relative links resolve (`../../../spec/PACKAGING.md`, `../../../docs/installation.md`).

### Documentation & Deployment
- [ ] `packaging/debian/README.md` cites spec §4.3 (recipe) + §2 (hidapi nuance).
- [ ] Commit message notes the `.desktop` is owned by P2.M6.T1.S1 (if the Task-1
      fallback created it) and that CI wiring is P1.M7.T1.S2.
- [ ] spec/PACKAGING.md NOT edited (read-only source of truth).

---

## Anti-Patterns to Avoid

- ❌ Don't add `-lhidapi-hidraw` to the `.deb` build — Debian's unified hidapi
  auto-selects hidraw; the flag is Arch-only (§2).
- ❌ Don't copy the Arch `qmkonnect.install` verbatim — it has the `&>` bashism and
  uses `groupadd`-era assumptions; translate to POSIX-sh + `addgroup`/`getent`.
- ❌ Don't add a `#DEBHELPER#` token — cargo-deb only needs it when it detects a
  `.service` unit; we ship `.service.template` (not detected), so a leftover
  literal `#DEBHELPER#` would be a bug.
- ❌ Don't create a `[package.metadata.generate-rpm]` block or CI workflow edits —
  those are P1.M7.T2 / P1.M7.T1.S2 (out of scope).
- ❌ Don't author `packaging/linux/xdg/qmkonnect.desktop` as a *deliverable* of this
  task — it's P2.M6.T1.S1's file; only create it as the Task-1 build-unblock
  fallback (content pinned by §4.7) and say so in the commit.
- ❌ Don't skip `cargo test` because "it's just packaging" — it's the regression
  guard; run it (fall back to `cargo check` only if build deps are absent, with a note).
- ❌ Don't edit `spec/PACKAGING.md` — it is the read-only source of truth.