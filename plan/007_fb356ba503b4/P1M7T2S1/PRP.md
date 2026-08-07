# PRP — P1.M7.T2.S1: `.rpm` package via cargo-generate-rpm (`[package.metadata.generate-rpm]` block + maintainer scripts)

---

## Goal

**Feature Goal**: Enable `cargo-generate-rpm` to produce a correct, installable RPM
package for QMKonnect by adding a `[package.metadata.generate-rpm]` block to
`Cargo.toml` and the supporting maintainer scripts, so the resulting
`target/generate-rpm/qmkonnect-<ver>-1.x86_64.rpm` ships the full Linux artifact
set (binary, hid-id udev helper, static udev rule, systemd user-service template,
XDG autostart `.desktop`, README, LICENSE) to the exact FHS paths mandated by
`spec/PACKAGING.md` §4, declares the Fedora runtime requires, and runs the
`%post`/`%postun` scriptlets that mirror the Debian/Arch install/remove hooks.

**Deliverable**:
1. A `[package.metadata.generate-rpm]` table + a `[package.metadata.generate-rpm.requires]`
   sub-table appended to `Cargo.toml` (the corrected, valid-TOML recipe — see
   "Spec Correction" below).
2. `packaging/rpm/postin` — RPM `%post` scriptlet (mirrors Debian `postinst`).
3. `packaging/rpm/postun` — RPM `%postun` scriptlet (mirrors Debian `postrm`,
   erase-guarded — see Gotcha #4).
4. `packaging/rpm/README.md` — Mode-A doc: how to build + install the `.rpm`.

**Success Definition**: `cargo install cargo-generate-rpm && cargo build --release
&& cargo generate-rpm` (run on a Fedora host, or `fedora-latest` in CI) emits
`target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm`; `rpm2cpio`/`cpio` inspection
shows all 7 assets at the correct target paths, correct header tags (`Requires`,
`License`, `Summary`, `URL`, `Vendor`), the versioned `hidapi >= 0.10` require,
and the two `%post`/`%postun` scriptlets embedded from the file refs; the build
used **no** `-lhidapi-hidraw` link flag; the maintainer scripts are POSIX-`sh`
and the `%postun` teardown is `$1 = 0` erase-guarded so upgrades don't tear down
the service.

## Why

- **F15 (community package-manager distribution)** calls for native `.rpm`
  alongside `.deb` and the other channels (`PACKAGING.md` §6, F15 row). The
  `.deb` sibling is already complete (P1.M7.T1.S1 — `[package.metadata.deb]` +
  `packaging/debian/` exist in the tree). Fedora/RHEL/Rocky/Alma/openSUSE users
  currently have only the generic tarball; a `.rpm` gives them `sudo dnf install
  …` / `sudo rpm -i …` one-liners with proper dependency resolution and the
  install/uninstall lifecycle the Arch + `.deb` packages already prove out.
- **The artifact set is shared**: every Linux channel installs the same binary +
  helper + static rule + service template + `.desktop` to the same FHS paths
  (`PACKAGING.md` §4 artifact table). The `.rpm` wraps that set in RPM metadata +
  the scriptlet lifecycle (mirror of `packaging/debian/postinst`/`postrm`, which
  themselves translate `packaging/linux/arch/qmkonnect.install`).
- **Scope boundary**: this task produces ONLY the `.rpm` build recipe + scripts +
  doc. Wiring the `.rpm` into the CI release pipeline (Fedora build job, release
  renaming) is **P1.M7.T2.S2**. The XDG `.desktop` file itself is **P2.M6.T1.S1**
  (consumed here, already present in the tree). The `.deb` metadata is
  **P1.M7.T1.S1** (consumed as the sibling pattern, already present). This task
  edits only `Cargo.toml` (append) and creates only `packaging/rpm/*`.

## What

User-/maintainer-visible behavior:
- `cargo generate-rpm` reads the new `[package.metadata.generate-rpm]` block +
  `[package.metadata.generate-rpm.requires]` sub-table and packages the release
  binaries + the 5 data assets + LICENSE into
  `target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm`.
- On `dnf install` / `rpm -i`, the `%post` scriptlet (embedded from
  `packaging/rpm/postin`) instantiates the user service, reloads udev, ensures
  the `input` group exists, globally enables the service, and prints zero-config
  next-steps — exactly like the Debian `postinst`.
- On `dnf remove` / `rpm -e` (erase, `$1 = 0`), the `%postun` scriptlet (embedded
  from `packaging/rpm/postun`) disables globally, stops/disables per-user
  instances, removes the instantiated service + any user-generated
  `/etc/udev/rules.d/99-qmkonnect.rules`, and reloads udev. On **upgrade**
  (`$1 = 2`) the teardown is skipped (so the upgrade doesn't rip out what the new
  package just installed).
- `packaging/rpm/README.md` documents `cargo build --release && cargo generate-rpm`
  + `sudo rpm -i` / `sudo dnf install …`.

### Success Criteria

- [ ] `[package.metadata.generate-rpm]` + `[package.metadata.generate-rpm.requires]`
      present in `Cargo.toml`, valid TOML, with the 7-asset array, `release = "1"`,
      `vendor`, explicit `url`, and `hidapi = ">= 0.10"` (note the mandatory space).
- [ ] `packaging/rpm/{postin,postun,README.md}` all created.
- [ ] Maintainer scripts start with `#!/bin/sh` and contain **no bashisms**
      (`&>` → `>/dev/null 2>&1`).
- [ ] `%postun` cleanup is guarded `if [ "$1" = "0" ]; then …; fi` (erase-only).
- [ ] `packaging/rpm/README.md` created (Mode-A build+install doc).
- [ ] `cargo generate-rpm` produces `target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm`
      whose payload contains the 7 assets at the §4 target paths and whose header
      carries the two scriptlets + the `hidapi >= 0.10` require + correct tags.
- [ ] Build uses **no** `-lhidapi-hidraw` (Fedora unified hidapi — §2).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` still green (no `.rs`
      touched; regression guard).

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?" — **YES.** The exact `[package.metadata.generate-rpm]`
> recipe is pinned below (a CORRECTED form of `spec/PACKAGING.md §4.4`, whose
> `require-local` line is invalid TOML — see the Spec Correction note); the
> maintainer-script logic is pinned in `packaging/debian/{postinst,postrm}` (the
> already-implemented Debian translations of `packaging/linux/arch/qmkonnect.install`);
> every asset path is enumerated and verified-present.

### Spec Correction (READ FIRST — the build WILL fail if you copy §4.4 verbatim)

`spec/PACKAGING.md §4.4` prints this line, which is **invalid TOML** and uses a
**nonexistent field name**:

```toml
# ❌ INVALID — do NOT copy this from §4.4
require-local = { "hidapi" >= "0.10", "libxdo", "zenity", "libnotify", "systemd" }
```

- `>=` is not a valid inline-table operator in TOML → `cargo`/`cargo generate-rpm`
  fails to parse `Cargo.toml`.
- `require-local` is **not a real cargo-generate-rpm field** (verified against the
  upstream README, fetched 2026-08-07; see `research/cargo-generate-rpm-readme.md`).

**The corrected form** (this PRP is authoritative for the build recipe): drop the
`require-local` line entirely and add a **dedicated sub-table**
`[package.metadata.generate-rpm.requires]` with map entries. See Task 2 for the
verbatim block. The sub-table is the upstream-documented way to express versioned
runtime requires (`hidapi = ">= 0.10"`, with a **mandatory space** after `>=`).
Everything else in §4.4 (the `assets` inline-table array, `post_install_script`/
`post_uninstall_script` file refs, `release`, `vendor`, `url`, `summary`,
`license`) is correct and is preserved below.

> Note: `spec/PACKAGING.md` is **read-only** (the source of truth is owned by
> humans). This PRP corrects the recipe at the IMPLEMENTATION layer (the Cargo.toml
> block we author) without editing the spec. If the spec is later updated to fix
> the `require-local` typo, the corrected block here already matches the intent.

### Documentation & References

```yaml
# MUST READ — the authoritative recipe source (COPY the corrected block in Task 2,
#             NOT the invalid require-local line from §4.4 verbatim)
- file: spec/PACKAGING.md
  section: "§4.4 .rpm via cargo-generate-rpm (packaging/rpm/) — NEW"
  why: Pins the .rpm contract: target Fedora/RHEL/Rocky/Alma/openSUSE; build
       WITHOUT -lhidapi-hidraw (Fedora unified hidapi — §2); the 5 assets + paths;
       postin/postun scriptlet intent; the output path + CI rename; the openSUSE
       shared-spec note (HIDAPI, libxdo-devel, libnotify-tools, zenity).
  critical: |
    The require-local line in §4.4 is INVALID TOML + a nonexistent field — use the
    [package.metadata.generate-rpm.requires] sub-table from Task 2 instead (see
    Spec Correction above). Everything else in §4.4 is correct.

# MUST READ — the Linux hidapi link nuance (the single most important build fact)
- file: spec/PACKAGING.md
  section: "§2 Features & Binaries → 'Linux hidapi link nuance (must-preserve)'"
  why: Explains WHY the .rpm must NOT pass -lhidapi-hidraw (Fedora ships a unified
       hidapi >=0.14 that folds both backends into libhidapi.so and auto-selects
       hidraw), while the Arch PKGBUILD MUST (Arch ships hidraw/libusb split).
       Getting this backwards breaks usage/usage_page matching at runtime.
  critical: Fedora/RHEL are EXPLICITLY listed in §2 as unified-hidapi distros —
            no flag, exactly like the .deb.

# MUST READ — the FHS artifact table (target paths the assets must land at)
- file: spec/PACKAGING.md
  section: "§4 Linux Packaging (artifact table)"
  why: The 7-row table pins the exact install path for every file. The .rpm asset
       dest paths must produce these paths (+ /usr/share/licenses/qmkonnect/LICENSE
       for the LICENSE, which the .deb ships via license-file but the .rpm must
       ship as an explicit asset — see Gotcha #5).
  pattern: |
    app binary           -> /usr/bin/qmkonnect
    udev helper          -> /usr/lib/udev/qmkonnect-hid-id
    static udev rule     -> /usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules
    service (template)   -> /usr/lib/systemd/user/qmkonnect.service.template
    service (instantiated)-> /usr/lib/systemd/user/qmkonnect.service  (postin writes)
    XDG autostart        -> /etc/xdg/autostart/qmkonnect.desktop
    docs                 -> /usr/share/doc/qmkonnect/
    license (.rpm-only)  -> /usr/share/licenses/qmkonnect/LICENSE

# MUST READ — the sibling .deb metadata (the pattern to mirror + the Cargo.toml
# insertion neighbor)
- file: Cargo.toml
  section: "[package.metadata.deb] (appended by P1.M7.T1.S1 — already in tree)"
  why: The .rpm block appends AFTER the .deb block at EOF. The .deb block shows
       the exact 6-asset set + paths the .rpm must mirror (the .rpm adds LICENSE
       as a 7th asset and switches to inline-table dest-path form).

# MUST READ — the maintainer-script logic to mirror (the Debian translations)
- file: packaging/debian/postinst
  why: postin mirrors this (instantiate service template → reload+trigger udev →
       ensure input group → systemctl --global enable → zero-config next-steps).
       Translate the ONE Debian-ism: addgroup → groupadd -r (Fedora/RHEL syntax).
- file: packaging/debian/postrm
  why: postun mirrors this (global disable → per-user stop/disable → rm
       instantiated service + /etc/udev/rules.d/99-qmkonnect.rules → udev reload),
       BUT wrapped in `if [ "$1" = "0" ]; then …; fi` so RPM upgrades ($1=2)
       don't tear down the service (Gotcha #4).

# MUST READ — the static udev rule (sets GROUP="input" → the input group must exist)
- file: packaging/linux/udev/69-qmkonnect-rawhid.rules
  why: The rule line `ENV{ID_QMKONNECT}=="1", GROUP="input", MODE="0660", …`
       assigns matched devices to the `input` group. postin must ensure that group
       exists (Fedora's systemd RPM ships it, but be idempotent like the .deb).

# MUST READ — the service template postin instantiates
- file: packaging/linux/systemd/qmkonnect.service.template
  why: postin copies this verbatim to /usr/lib/systemd/user/qmkonnect.service,
       then `systemctl --global enable`s it.

# REFERENCE — sibling packaging README (mirror its style for packaging/rpm/README.md)
- file: packaging/debian/README.md
  why: Style template: H1 title, "What this is", fenced "Build"/"Install" blocks,
       "What it installs" table, hidapi-link note, relative cross-links to
       spec/PACKAGING.md + docs/installation.md. The .rpm README mirrors it with
       dnf/rpm commands + the Fedora require set.

# EXTERNAL — cargo-generate-rpm config semantics (verified against upstream README)
- url: https://github.com/cat-in-136/cargo-generate-rpm#configuration
  why: Canonical [package.metadata.generate-rpm] field reference. Confirms:
       assets inline-table form {source,dest,mode}; versioned requires live in a
       [package.metadata.generate-rpm.requires] sub-table with map entries +
       mandatory space after the operator; post_*_script accept a STRING OR FILE
       PATH (file path ⇒ contents embedded as the %post/%postun scriptlet); cargo
       generate-rpm does NOT auto-build (run cargo build --release first); output
       target/generate-rpm/<name>-<version>-<release>.<arch>.rpm; no license-file
       mechanism (LICENSE must be an explicit asset).
  critical: |
    - require-local / require are NOT the field names. Use `requires` (plural,
      inline list) OR the `[package.metadata.generate-rpm.requires]` sub-table
      (map form) for versioned requires. This PRP uses the sub-table (Task 2).
    - `">= 0.10"` (space) is valid; `">=0.10"` (no space) is REJECTED.
    - auto-req is ON by default → the .rpm auto-detects libfoo.so.N() requires.
      Keep it on; our explicit requires table adds package-level + version floor.
    - require-sh defaults true (/bin/sh auto-added) — keep it (our scripts are sh).
```

### Current Codebase Tree (relevant slice)

```bash
Cargo.toml                                   # [package] name=qmkonnect v0.2.8; [package.metadata.deb] ALREADY present (P1.M7.T1.S1); NO [package.metadata.generate-rpm] yet
.cargo/config.toml                           # windows-msvc +crt-static only (Linux untouched — good, no hidraw flag)
packaging/
├── linux/
│   ├── udev/69-qmkonnect-rawhid.rules       # ASSET ✓ (ships verbatim; sets GROUP="input")
│   ├── systemd/qmkonnect.service.template   # ASSET ✓ (postin instantiates)
│   ├── xdg/qmkonnect.desktop                # ASSET ✓ (PRESENT — P2.M6.T1.S1 landed)
│   └── arch/qmkonnect.install               # LOGIC SOURCE (original pacman hooks)
├── debian/                                  # SIBLING (P1.M7.T1.S1 — landed): postinst/postrm/prerm/README.md/long-description.txt
├── rpm/                                     # ❌ DOES NOT EXIST — this task creates it
└── (aur/, nix/, homebrew/, scoop/, winget/, asdf/, windows/, macos/ — existing channels)
README.md  LICENSE                           # ASSETS ✓ (README -> doc; LICENSE -> /usr/share/licenses/)
target/release/{qmkonnect,qmkonnect-hid-id}  # ASSETS ✓ (built by cargo build --release)
spec/PACKAGING.md                            # READ-ONLY source of truth (§2, §4, §4.4)
```

### Desired Codebase Tree (files this task creates/edits)

```bash
Cargo.toml                                   # EDIT: append [package.metadata.generate-rpm] + [package.metadata.generate-rpm.requires] at EOF
packaging/rpm/                               # NEW dir
├── postin                                   # NEW — POSIX-sh %post scriptlet (mirrors debian/postinst)
├── postun                                   # NEW — POSIX-sh %postun scriptlet (mirrors debian/postrm, $1=0-guarded)
└── README.md                                # NEW — Mode-A build+install doc
# (all assets — binary/helper/rule/template/.desktop/README/LICENSE — already exist in the tree)
```

### Known Gotchas of our codebase & library quirks

```bash
# CRITICAL (build correctness): the .rpm build must NOT pass -lhidapi-hidraw.
# Fedora/RHEL ship a UNIFIED hidapi (>=0.14) in libhidapi.so that auto-selects
# the hidraw backend at runtime; usage/usage_page matching works without the flag.
# (Arch splits hidraw/libusb; only the Arch PKGBUILD sets the flag. spec §2.)
# => `cargo build --release && cargo generate-rpm` is correct as-is; do NOT add a
#    RUSTFLAGS hidraw flag to the .rpm path anywhere.

# CRITICAL (spec §4.4 has INVALID TOML — see Spec Correction): the printed line
#   require-local = { "hidapi" >= "0.10", "libxdo", ... }
# is unparseable TOML + a nonexistent field. Use the
# [package.metadata.generate-rpm.requires] sub-table from Task 2. Do NOT copy the
# §4.4 require-local line verbatim — `cargo generate-rpm` will fail to parse
# Cargo.toml.

# CRITICAL (RPM %postun upgrade semantics — Gotcha #4): unlike Debian postrm
# (which only runs on remove/purge), RPM %postun ALSO runs on UPGRADE ($1=2),
# for the OLD package, right after the NEW package is installed. If postun
# unconditionally tears down the service + 99- rule, an UPGRADE removes what the
# new package just installed. => Guard the entire postun cleanup with
#   if [ "$1" = "0" ]; then …; fi   (erase-only). %post (postin) needs NO guard
# (instantiate/reload/global-enable is idempotent + desirable on upgrade).

# CRITICAL (POSIX sh): RPM scriptlets run under /bin/sh. The Debian postrm
# already translated the Arch bashism `&>` → `>/dev/null 2>&1`; carry that over.
# No `&>` anywhere in postin/postun.

# CRITICAL (input group idempotency): the static udev rule sets GROUP="input".
# Fedora's systemd RPM ships the `input` group, but be idempotent like the .deb.
# Use Fedora/RHEL syntax: `getent group input >/dev/null 2>&1 || groupadd -r input`
# (groupadd -r = system group; do NOT use Debian's addgroup).

# CRITICAL (LICENSE must be an explicit asset): cargo-generate-rpm has NO
# license-file mechanism (unlike cargo-deb's `license-file = ["LICENSE","0"]`).
# The `license = "MIT"` field only sets the RPM License: tag. To ship the LICENSE
# text, add it as an asset: dest "/usr/share/licenses/qmkonnect/LICENSE", mode 644.
# (This is the 7th asset — the "same 7 files as .deb" the contract refers to.)

# CRITICAL (cargo generate-rpm does NOT auto-build): run `cargo build --release`
# FIRST, then `cargo generate-rpm`. Our [profile.release] strip = true handles
# stripping (no manual `strip -s` needed). cargo generate-rpm reads
# target/release/{qmkonnect,qmkonnect-hid-id} as asset sources.

# GOTCHA (host build env): this dev box is ARCH (split hidapi). A LOCAL
# `cargo build --release` here WITHOUT -lhidapi-hidraw mis-links the hidapi crate
# against Arch's split libs. For LOCAL STRUCTURAL validation only, set
#   RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release
# for the binary build step, then `cargo generate-rpm`. The resulting .rpm is
# structurally valid (right files/requires/scripts) but built against Arch glibc
# — NOT for release. The AUTHORITATIVE build is Fedora CI (P1.M7.T2.S2).
# cargo-generate-rpm itself (the rpm-packaging step) runs on any host.

# GOTCHA (cargo-generate-rpm not installed): run `cargo install cargo-generate-rpm`
# first. It builds the .rpm on any Unix host via the `rpm` crate — it does NOT
# need rpmbuild installed.

# GOTCHA (auto-req is ON by default): cargo-generate-rpm auto-detects shared-lib
# requires (libhidapi.so.0()(64bit), etc.) via builtin ldd. This is GOOD — keep
# it on (do NOT set auto-req = "no"). Our explicit [package.metadata.generate-rpm.requires]
# table adds package-level requires + a version floor; they coexist.

# GOTCHA (url must be explicit): Cargo.toml has NO [package] homepage or
# repository field, so cargo-generate-rpm cannot fall back for `url`. Set
# url = "https://github.com/dabstractor/qmkonnect" explicitly (Task 2).
```

## Implementation Blueprint

### Data models and structure

_None._ This is packaging metadata + shell scripts + a doc. No Rust types,
schemas, or runtime models change. The only edit to `Cargo.toml` is an appended
metadata table + sub-table (does not affect the build fingerprint for code).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY the build preconditions (assets all present)
  - RUN: test -f packaging/linux/xdg/qmkonnect.desktop && echo "desktop OK"
  - RUN: test -f target/release/qmkonnect && test -f target/release/qmkonnect-hid-id && echo "bins OK"
  - RUN: test -f packaging/linux/udev/69-qmkonnect-rawhid.rules && test -f packaging/linux/systemd/qmkonnect.service.template && echo "rule+template OK"
  - RUN: test -f README.md && test -f LICENSE && echo "docs+license OK"
  - EXPECT: all OK (every asset the .rpm references already exists in the tree).
    The .desktop (P2.M6.T1.S1) and the .deb metadata (P1.M7.T1.S1) both landed,
    so all preconditions are satisfied. If any is absent, STOP and flag it — do
    NOT author the asset here (each is owned by its own task).
  - DEPENDENCIES: none.

Task 2: EDIT Cargo.toml — append the [package.metadata.generate-rpm] table + the
        [package.metadata.generate-rpm.requires] sub-table at EOF
  - APPEND at END of file (after the existing [package.metadata.deb] block — it is
    currently the last table in the file). Add a NEW top-level table; do NOT nest
    it inside any existing table.
  - ADD a leading comment block (mirror the .deb block's comment style) explaining:
    Fedora/RHEL/Rocky/Alma/openSUSE target; authoritative build is Fedora CI
    (P1.M7.T2.S2); built WITHOUT -lhidapi-hidraw (Fedora unified hidapi, §2);
    the .deb sibling is P1.M7.T1.S1; the requires sub-table replaces spec §4.4's
    invalid require-local line.
  - USE EXACTLY this block (the corrected recipe — valid TOML):

      # .rpm via cargo-generate-rpm (spec/PACKAGING.md §4.4). Targets
      # Fedora/RHEL/Rocky/Alma/openSUSE — the authoritative build is Fedora CI
      # (P1.M7.T2.S2). Built WITHOUT -lhidapi-hidraw: Fedora's unified hidapi
      # (>=0.14) auto-selects the hidraw backend at runtime (spec §2). The .deb
      # sibling ([package.metadata.deb]) is P1.M7.T1.S1.
      #
      # NOTE: spec §4.4 prints a `require-local = { "hidapi" >= "0.10", … }` line
      # that is INVALID TOML + a nonexistent cargo-generate-rpm field. Versioned
      # runtime requires go in the [package.metadata.generate-rpm.requires]
      # sub-table below (upstream-documented form; space after `>=` is mandatory).
      [package.metadata.generate-rpm]
      name = "qmkonnect"
      license = "MIT"
      summary = "Cross-platform window activity notifier for QMK keyboards"
      release = "1"
      vendor = "Mulletware"
      url = "https://github.com/dabstractor/qmkonnect"
      post_install_script = "packaging/rpm/postin"
      post_uninstall_script = "packaging/rpm/postun"
      assets = [
        { source = "target/release/qmkonnect",                              dest = "/usr/bin/qmkonnect",                                     mode = "755" },
        { source = "target/release/qmkonnect-hid-id",                       dest = "/usr/lib/udev/qmkonnect-hid-id",                         mode = "755" },
        { source = "packaging/linux/udev/69-qmkonnect-rawhid.rules",        dest = "/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules",        mode = "644" },
        { source = "packaging/linux/systemd/qmkonnect.service.template",    dest = "/usr/lib/systemd/user/qmkonnect.service.template",        mode = "644" },
        { source = "packaging/linux/xdg/qmkonnect.desktop",                 dest = "/etc/xdg/autostart/qmkonnect.desktop",                   mode = "644" },
        { source = "README.md",                                             dest = "/usr/share/doc/qmkonnect/README.md",                     mode = "644" },
        { source = "LICENSE",                                               dest = "/usr/share/licenses/qmkonnect/LICENSE",                  mode = "644" },
      ]

      [package.metadata.generate-rpm.requires]
      hidapi = ">= 0.10"
      libxdo = "*"
      zenity = "*"
      libnotify = "*"
      systemd = "*"

  - PRESERVE: every existing field/table (the [package] block, [dependencies],
    [features], [[bin]], [profile.release], AND the [package.metadata.deb] block).
    This is a pure append; touch NOTHING else.
  - WHY each field:
      * name/license/summary — explicit (would otherwise fall back to package.*
        which would also work; explicit is clearer and pins the RPM Summary to one
        line < 80 chars).
      * release = "1" — appears in the output filename (qmkonnect-0.2.8-1.x86_64.rpm).
      * vendor — optional but the contract/spec set it; keeps the header complete.
      * url — MUST be explicit (Cargo.toml has no homepage/repository; Gotcha).
      * post_install_script / post_uninstall_script = file paths — cargo-generate-rpm
        reads each file and embeds its contents as the %post / %postun scriptlet
        (verified against the upstream README).
      * assets — inline-table form {source,dest,mode}; dest is the FULL install
        path (not a dir, unlike cargo-deb's target-dir form). 7 entries = the 6
        the .deb ships + LICENSE (the .rpm has no license-file mechanism).
      * [package.metadata.generate-rpm.requires] sub-table — the upstream-correct
        place for versioned requires. hidapi = ">= 0.10" (space after >=); the
        rest use "*" (any version). auto-req adds the libfoo.so.N() requires on top.
  - DEPENDENCIES: Task 1 (assets exist) for a successful build.

Task 3: CREATE packaging/rpm/postin  (RPM %post scriptlet; mirrors debian/postinst)
  - SHEBANG: #!/bin/sh  then  set -e
  - HEADER COMMENT: note this is %post (runs on install AND upgrade; $1: 1=install,
    2=upgrade), mirrors packaging/debian/postinst translated to Fedora conventions
    (groupadd -r instead of addgroup), spec §4.4.
  - LOGIC (port from packaging/debian/postinst):
      1. Instantiate the user service from its template:
           if [ -f /usr/lib/systemd/user/qmkonnect.service.template ]; then
             install -m644 \
               /usr/lib/systemd/user/qmkonnect.service.template \
               /usr/lib/systemd/user/qmkonnect.service
           fi
      2. Reload + trigger udev:
           udevadm control --reload-rules
           udevadm trigger
      3. Ensure the input group exists (idempotent — Fedora's systemd RPM ships it;
         use Fedora/RHEL groupadd -r, NOT Debian addgroup):
           getent group input >/dev/null 2>&1 || groupadd -r input
      4. Globally enable the user service (best-effort):
           systemctl --global enable qmkonnect.service >/dev/null 2>&1 || true
      5. Print the zero-config next-steps heredoc (copy the cat <<'EOF' ... EOF
         block verbatim from packaging/debian/postinst — identical message).
  - POSIX CHECK: no `&>`. Use `>/dev/null 2>&1` everywhere.
  - NO $1 guard needed (the logic is idempotent + desirable on upgrade).

Task 4: CREATE packaging/rpm/postun  (RPM %postun scriptlet; mirrors debian/postrm,
        ERASE-GUARDED)
  - SHEBANG: #!/bin/sh  then  set -e
  - HEADER COMMENT: note this is %postun (runs after erase AND after upgrade; $1:
    1=erase, 2=upgrade); the cleanup is GUARDED on "$1" = "0" (erase) so an upgrade
    ($1=2) does NOT tear down the service + rules the new package just installed;
    mirrors packaging/debian/postrm otherwise; spec §4.4.
  - LOGIC:
      # Only do teardown on complete removal, not on upgrade.
      if [ "$1" = "0" ]; then
        echo "Removing QMKonnect..."
        # 1. Disable globally (best-effort)
        systemctl --global disable qmkonnect.service >/dev/null 2>&1 || true
        # 2. Stop + disable per-user instances for each /home/* user
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
        # 3. Remove generated/user-created files (rpm already removed package-owned
        #    files like the static rule + helper + template on erase)
        rm -f /etc/udev/rules.d/99-qmkonnect.rules
        rm -f /usr/lib/systemd/user/qmkonnect.service
        # 4. Reload udev rules
        udevadm control --reload-rules
        udevadm trigger
        echo "QMKonnect has been successfully removed."
      fi
      exit 0
  - POSIX CHECK: no `&>` (use `>/dev/null 2>&1` — the Debian postrm already did
    this translation; carry it over). The `if [ "$1" = "0" ]` is the RPM-specific
    addition over the Debian postrm.
  - NO preun (pre_uninstall_script) needed — mirrors the Debian prerm no-op; the
    contract asks only for postin/postun.

Task 5: CREATE packaging/rpm/README.md  (Mode-A build + install doc)
  - STYLE: mirror packaging/debian/README.md (H1, "What this is", fenced
    "Build"/"Install" blocks, "What it installs" table, hidapi-link note, relative
    cross-links). Adapt commands to dnf/rpm + the Fedora require set.
  - RELATIVE LINKS from packaging/rpm/ (3 levels up to repo root):
      spec:     ../../../spec/PACKAGING.md     (cite §4.4 + §4 + §2)
      install:  ../../../docs/installation.md
      readme:   ../../../README.md
  - CONTENT must cover:
      * What it is: the native Fedora/RHEL/Rocky/Alma/openSUSE package built with
        cargo-generate-rpm from a [package.metadata.generate-rpm] block in
        Cargo.toml (spec §4.4). The .rpm sibling of the .deb (P1.M7.T1.S1).
      * Build (on Fedora — unified hidapi, NO -lhidapi-hidraw):
          cargo install cargo-generate-rpm     # one-time
          cargo build --release                # produce target/release/{qmkonnect,qmkonnect-hid-id}
          cargo generate-rpm                   # produce target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm
        (NOTE: cargo generate-rpm does NOT build for you — run cargo build first.)
        Output: target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm
        (CI builds on Fedora; the release artifact is renamed
        qmkonnect-<ver>-linux-x86_64.rpm — owned by P1.M7.T2.S2.)
      * Build dependencies (dnf): hidapi-devel libxdo-devel pkgconf-pkg-config
        (plus the full GTK/X11 tray stack the binary links — gtk3-devel glib2-devel
        libappindicator-gtk3-devel libX11-devel libxcb-devel systemd-devel; CI
        installs the full set, see P1.M7.T2.S2).
      * Install:
          # Option A — dnf (auto-resolves Requires):
          sudo dnf install target/generate-rpm/qmkonnect-*.rpm
          # Option B — rpm (does NOT auto-resolve; install deps yourself):
          sudo rpm -i target/generate-rpm/qmkonnect-*.rpm
      * Runtime requires (auto-resolved by dnf): hidapi (>= 0.10), libxdo, zenity,
        libnotify, systemd. (cargo-generate-rpm's auto-req also adds the
        libfoo.so.N() library-level requires.)
      * openSUSE note: shares this spec with package names HIDAPI, libxdo-devel,
        libnotify-tools, zenity (spec §4.4); an OBS submit is a community follow-on.
      * "What it installs" table: same 6 paths as the .deb + LICENSE at
        /usr/share/licenses/qmkonnect/LICENSE.
      * Install/uninstall hooks: postin (%post) = instantiate service → reload udev
        → ensure input group (groupadd -r) → systemctl --global enable → next-steps;
        postun (%postun) = erase-guarded ($1=0) disable + per-user stop + remove
        instantiated service + 99- rule + udev reload. (On upgrade, postun teardown
        is skipped so the upgrade doesn't tear down the new install.)
      * hidapi-link note: NO -lhidapi-hidraw (Fedora unified hidapi ≥0.14
        auto-selects hidraw; the flag is Arch-only). Cite spec §2.

Task 6: BUILD + VALIDATE the .rpm
  - INSTALL tool: cargo install cargo-generate-rpm
  - BUILD: cargo build --release && cargo generate-rpm
    (If the local host is ARCH and the binary link fails on the split hidapi lib,
     set RUSTFLAGS="-C link-arg=-lhidapi-hidraw" for the LOCAL `cargo build` step
     ONLY, then `cargo generate-rpm`. Document this is an Arch-only local-validation
     workaround; the authoritative build is Fedora CI — P1.M7.T2.S2. Do NOT bake
     the flag into the recipe.)
  - INSPECT without rpmbuild (works on any host with rpm2cpio/cpio, or `rpm`):
      RPM=target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm
      test -f "$RPM" && echo "PASS rpm produced: $RPM" || { echo "FAIL no rpm"; exit 1; }
      # Payload file list:
      rpm2cpio "$RPM" | cpio -tv 2>/dev/null | grep -E 'usr/bin/qmkonnect$|usr/lib/udev/qmkonnect-hid-id$|usr/lib/udev/rules.d/69-qmkonnect-rawhid\.rules$|usr/lib/systemd/user/qmkonnect\.service\.template$|etc/xdg/autostart/qmkonnect\.desktop$|usr/share/doc/qmkonnect/README\.md$|usr/share/licenses/qmkonnect/LICENSE$'
      # Header tags + requires + scriptlets (needs `rpm`):
      rpm -qp --requires "$RPM" | grep -E '^hidapi|libxdo|zenity|libnotify|systemd|/bin/sh'
      rpm -qp --scripts "$RPM"   # shows postin + postun embedded
      rpm -qpi "$RPM"            # Summary, License, URL, Vendor, etc.
  - ASSERT (see Validation Loop Level 3 for the exact checks):
      * payload contains the 7 dest paths.
      * header Requires includes `hidapi >= 0.10` (and libxdo/zenity/libnotify/systemd
        + the auto-detected libfoo.so.N() + /bin/sh).
      * `rpm -qp --scripts` shows the postin + postun contents (embedded from the
        file refs).
      * Summary = "Cross-platform window activity notifier for QMK keyboards",
        License = MIT, URL = https://github.com/dabstractor/qmkonnect.
  - REGRESSION: cargo test --bin qmkonnect -- --test-threads=1 (no .rs changed).
```

### Implementation Patterns & Key Details

```sh
# The canonical POSIX-sh postin skeleton (Task 3) — note every redirect is
# `>/dev/null 2>&1`, NEVER `&>`; and groupadd -r (Fedora), NOT addgroup (Debian):
#!/bin/sh
# postin: %post scriptlet (runs on install AND upgrade). Mirrors
# packaging/debian/postinst. See spec/PACKAGING.md §4.4.
set -e

echo "Configuring QMKonnect..."

# 1. Instantiate the user service from its template (no VID/PID substitution).
if [ -f /usr/lib/systemd/user/qmkonnect.service.template ]; then
  install -m644 \
    /usr/lib/systemd/user/qmkonnect.service.template \
    /usr/lib/systemd/user/qmkonnect.service
fi

# 2. Load the static usage-page udev rule so default QMK boards Just Work.
udevadm control --reload-rules
udevadm trigger

# 3. Ensure the input group exists (the udev rule sets GROUP="input"; Fedora's
#    systemd RPM ships it; be idempotent). groupadd -r = system group (Fedora/RHEL).
getent group input >/dev/null 2>&1 || groupadd -r input

# 4. Enable the user service globally (best-effort).
systemctl --global enable qmkonnect.service >/dev/null 2>&1 || true

# 5. Zero-config next-steps (verbatim from packaging/debian/postinst).
cat <<'EOF'

QMKonnect installed. Default QMK keyboards need NO configuration: QMKonnect
auto-discovers them by the standard Raw HID usage page (0xFF60 / 0x61), and the
shipped static udev rule already grants permissions (no --reload, no sudo).

Only if you need to disambiguate among multiple QMK keyboards, or target a
board that overrode RAW_USAGE_PAGE/RAW_USAGE_ID in firmware:
  1. Create / edit  ~/.config/qmkonnect/config.toml  and set
     vendor_id / product_id (and optionally usage_page / usage):
       qmkonnect -c            # writes a commented-out default config
  2. Generate + install the matching on-demand udev rule:
       sudo qmkonnect -r

The per-user service starts automatically at login once a matching device is
present. To start it right now:
       systemctl --user start qmkonnect.service

EOF

# ── postun: the $1 = 0 erase-guard is the ONE structural difference from ──
# the Debian postrm. Without it, an `dnf upgrade` runs %postun for the old
# package after the new one lands and rips out the service + 99- rule.
#!/bin/sh
# postun: %postun scriptlet (runs after erase AND after upgrade; $1: 1=erase,
# 2=upgrade). Cleanup is GUARDED on "$1" = "0" so an upgrade doesn't tear down
# the new install. Mirrors packaging/debian/postrm otherwise. spec §4.4.
set -e
if [ "$1" = "0" ]; then
  # ... disable globally, per-user stop/disable, rm instantiated service +
  # /etc/udev/rules.d/99-qmkonnect.rules, udev reload (Task 4) ...
fi
exit 0
```

### Integration Points

```yaml
CARGO.TOML:
  - append: "[package.metadata.generate-rpm]" + "[package.metadata.generate-rpm.requires]"
    top-level tables (Task 2) — the corrected recipe (NOT the §4.4 require-local line)
  - preserve: [package], [dependencies], [features], [[bin]], [profile.release],
    [package.metadata.deb]
  - DO NOT add a second [package.metadata.deb] or any CI/workflow edit.

PACKAGING TREE:
  - create dir: packaging/rpm/  (postin, postun, README.md)

PRECONDITIONS (consumed, not owned here — all already in tree):
  - packaging/linux/xdg/qmkonnect.desktop     (P2.M6.T1.S1)
  - packaging/linux/udev/69-qmkonnect-rawhid.rules, packaging/linux/systemd/qmkonnect.service.template
  - packaging/debian/{postinst,postrm}        (P1.M7.T1.S1 — the logic to mirror)
  - target/release/{qmkonnect,qmkonnect-hid-id}, README.md, LICENSE

NO CHANGES TO:
  - any .rs, .github/workflows/* (CI is P1.M7.T2.S2), release.toml, .cargo/config.toml
  - spec/PACKAGING.md (read-only source of truth)
  - the Arch PKGBUILDs / AUR / Nix / .deb / other channels
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Cargo.toml still parses (the metadata block + requires sub-table are valid TOML):
cargo metadata --no-deps --format-version 1 >/dev/null && echo "PASS cargo metadata"
# CRITICAL: this FAILS today if you copied §4.4's require-local line verbatim —
# the corrected sub-table in Task 2 is what makes it parse.

# Maintainer scripts are POSIX-sh clean (if shellcheck is available):
shellcheck packaging/rpm/postin packaging/rpm/postun \
  || echo "(shellcheck not installed — visual review for bashisms)"
# CRITICAL manual check: grep MUST return nothing:
grep -nE '&>' packaging/rpm/postin packaging/rpm/postun \
  && echo "FAIL: bashism &> present" || echo "PASS: no &> bashism"
# Shebang check:
head -1 packaging/rpm/postin packaging/rpm/postun
# Expected: #!/bin/sh on both.

# postun MUST be erase-guarded (Gotcha #4):
grep -q 'if \[ "\$1" = "0" \]' packaging/rpm/postun && echo "PASS postun \$1=0 guard" \
  || echo "FAIL: postun missing \$1=0 erase guard"

# Expected: cargo metadata exits 0; no &> matches; shebangs #!/bin/sh; guard present.
```

### Level 2: Asset Resolution (build-input correctness)

```bash
# Every asset source path in the Task-2 array must resolve:
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
# Expected: all OK (Task 1 already verified). If any ABSENT, STOP — flag the
# owning task; do NOT author the asset here.
```

### Level 3: .rpm Build + Structure (the core gate)

```bash
# Install cargo-generate-rpm if missing:
cargo generate-rpm --version 2>/dev/null || cargo install cargo-generate-rpm

# Build the binary first (cargo generate-rpm does NOT build). NO -lhidapi-hidraw.
cargo build --release
# Arch-host fallback ONLY if the link fails on the split hidapi lib:
#   RUSTFLAGS="-C link-arg=-lhidapi-hidraw" cargo build --release
# (Document this is an Arch-only local-validation workaround; authoritative = Fedora CI.)

# Package:
cargo generate-rpm

RPM=target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm
test -f "$RPM" && echo "PASS rpm produced: $RPM" || { echo "FAIL no rpm"; exit 1; }

# Payload: assert the 7 assets land at the §4 paths (rpm2cpio works on any host):
rpm2cpio "$RPM" | cpio -tv 2>/dev/null \
  | grep -E 'usr/bin/qmkonnect$|usr/lib/udev/qmkonnect-hid-id$|usr/lib/udev/rules.d/69-qmkonnect-rawhid\.rules$|usr/lib/systemd/user/qmkonnect\.service\.template$|etc/xdg/autostart/qmkonnect\.desktop$|usr/share/doc/qmkonnect/README\.md$|usr/share/licenses/qmkonnect/LICENSE$'
# Expected: 7 matching lines.

# Header requires (needs `rpm`; on a non-Fedora host install `rpm` or skip):
if command -v rpm >/dev/null; then
  rpm -qp --requires "$RPM" | grep -E '^hidapi|^libxdo|^zenity|^libnotify|^systemd|/bin/sh' \
    && echo "(+ auto-detected libfoo.so.N() requires)"
  # Expected: hidapi >= 0.10, libxdo, zenity, libnotify, systemd, /bin/sh
  #           (and the ldd-detected library requires).

  # Scriptlets embedded from the file refs:
  rpm -qp --scripts "$RPM"
  # Expected: a "postinstall program:/bin/sh" + "postuninstall program:/bin/sh" block
  #           whose bodies are the contents of packaging/rpm/postin + postun.

  # Header tags:
  rpm -qpi "$RPM" | grep -E '^Summary|^License|^URL|^Vendor|^Name|^Version|^Release'
  # Expected: Summary: Cross-platform window activity notifier for QMK keyboards
  #           License: MIT
  #           URL: https://github.com/dabstractor/qmkonnect
  #           Vendor: Mulletware
  #           Release: 1
else
  echo "(rpm(1) not installed on this host — inspect on Fedora or with rpm2cpio above)"
fi
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
- [ ] Level 1: `cargo metadata` exits 0 (valid TOML); no `&>` bashism; shebangs
      `#!/bin/sh`; postun has the `$1 = "0"` guard.
- [ ] Level 2: all 7 asset sources resolve.
- [ ] Level 3: `cargo build --release && cargo generate-rpm` produces
      `target/generate-rpm/qmkonnect-0.2.8-1.x86_64.rpm`; payload has the 7 dest
      paths; header has `hidapi >= 0.10` + libxdo/zenity/libnotify/systemd +
      `/bin/sh`; `rpm -qp --scripts` shows postin + postun embedded; tags correct.
- [ ] Level 4: `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] Build used **no** `-lhidapi-hidraw` in the recipe (any hidraw flag was an
      Arch-only local-validation workaround, clearly documented).

### Feature Validation
- [ ] `[package.metadata.generate-rpm]` + `[package.metadata.generate-rpm.requires]`
      present in Cargo.toml, valid TOML, with the corrected recipe (NOT §4.4's
      `require-local` line).
- [ ] `packaging/rpm/{postin,postun,README.md}` created.
- [ ] postin logic mirrors Debian `postinst` (instantiate, udev reload,
      `--global enable`, ensure `input` group via `groupadd -r`, next-steps).
- [ ] postun logic mirrors Debian `postrm` but is `$1 = "0"` erase-guarded
      (global disable, per-user stop/disable, remove instantiated service + 99-
      rule, udev reload — all inside the guard).
- [ ] `packaging/rpm/README.md` documents `cargo build --release && cargo generate-rpm`
      + `sudo dnf install` / `sudo rpm -i`.

### Code Quality Validation
- [ ] Maintainer scripts are POSIX-`sh` (no bashisms).
- [ ] postun is erase-guarded (RPM upgrade-correctness).
- [ ] `input`-group enablement is idempotent (`getent … || groupadd -r`).
- [ ] LICENSE shipped as an explicit asset to `/usr/share/licenses/qmkonnect/LICENSE`.
- [ ] Cargo.toml edit is a pure append; no existing field/table disturbed.
- [ ] README relative links resolve (`../../../spec/PACKAGING.md`, `../../../docs/installation.md`).

### Documentation & Deployment
- [ ] `packaging/rpm/README.md` cites spec §4.4 (recipe) + §4 (paths) + §2 (hidapi).
- [ ] The Cargo.toml comment block notes that §4.4's `require-local` line is invalid
      and the `[...generate-rpm.requires]` sub-table is the corrected form.
- [ ] Commit message notes: (a) the spec-correction; (b) the build precondition
      is the shared artifact set (+ P2.M6.T1.S1 .desktop); (c) CI wiring is
      P1.M7.T2.S2; (d) the postun `$1=0` guard is the RPM-correct deviation from
      the Debian postrm.
- [ ] spec/PACKAGING.md NOT edited (read-only source of truth).

---

## Anti-Patterns to Avoid

- ❌ Don't copy `spec/PACKAGING.md §4.4`'s `require-local = { "hidapi" >= "0.10", … }`
  line verbatim — it is **invalid TOML** + a nonexistent field. Use the
  `[package.metadata.generate-rpm.requires]` sub-table (Task 2).
- ❌ Don't forget the **space** after `>=` — `hidapi = ">= 0.10"` is valid;
  `hidapi = ">=0.10"` is REJECTED by cargo-generate-rpm's version parser.
- ❌ Don't ship the LICENSE via a `license-file = [...]` field — cargo-generate-rpm
  has no such mechanism (that's cargo-deb). Add LICENSE as an explicit asset.
- ❌ Don't omit the `$1 = "0"` guard on postun — without it a `dnf upgrade` tears
  down the service + rules the new package just installed.
- ❌ Don't run `cargo generate-rpm` without building first — it does NOT auto-build
  (run `cargo build --release` first).
- ❌ Don't add `-lhidapi-hidraw` to the `.rpm` build — Fedora's unified hidapi
  auto-selects hidraw; the flag is Arch-only (§2).
- ❌ Don't use Debian's `addgroup` in postin — Fedora/RHEL use `groupadd -r`.
- ❌ Don't copy the Debian `postinst`/`postrm` `&>` redirect — the Debian files
  already use `>/dev/null 2>&1`; keep it that way (RPM scriptlets run under sh).
- ❌ Don't set `auto-req = "no"` — auto-require detection adds the correct
  libfoo.so.N() requires. Keep it on; our explicit requires table coexists.
- ❌ Don't create a `[package.metadata.deb]` edit, CI workflow edit, or
  `packaging/debian/*` — those are P1.M7.T1.S1 / P1.M7.T2.S2 (out of scope).
- ❌ Don't skip `cargo test` because "it's just packaging" — it's the regression
  guard; run it (fall back to `cargo check` only if build deps are absent, with a note).
- ❌ Don't edit `spec/PACKAGING.md` — it is the read-only source of truth.

---

## Confidence Score: 9/10

**Why 9, not 10:** The exact metadata block (corrected from the spec's invalid
`require-local` line), the requires sub-table syntax (with the mandatory space),
the asset/dest/mode form, the `post_install_script`/`post_uninstall_script`
file-ref embedding, the "build first, then generate-rpm" flow, the output-path
naming, and every gotcha are fully specified and verified against the upstream
cargo-generate-rpm README (fetched 2026-08-07; saved in
`research/cargo-generate-rpm-readme.md`). The maintainer-script logic is a direct,
field-tested mirror of the already-landed `packaging/debian/{postinst,postrm}`
plus the one RPM-correct `$1 = "0"` guard. The -1 is for: (a) end-to-end
validation that can only fully run on Fedora (local Arch validation proves
structure, not glibc/distribution fit — the authoritative build is Fedora CI,
P1.M7.T2.S2); and (b) the spec §4.4 correction introduces a small judgment call
(validated against upstream, but the human owner may prefer a different valid form
— e.g. an inline `requires = [...]` list — though the sub-table is the
upstream-documented canonical form for versioned requires). One-pass implementation
success is very high: the task is a single Cargo.toml append + two short shell
scripts + one README, with a complete, copy-able, verified recipe.