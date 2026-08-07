# PRP — P2.M6.T1.S1: Create `packaging/linux/xdg/qmkonnect.desktop` + Ship in All Linux Packages

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Files CREATED:** `packaging/linux/xdg/qmkonnect.desktop` (the F17 universal
>   autostart entry — the PRIMARY deliverable + a prerequisite for 4 sibling tasks).
> **Files MODIFIED:** `.github/workflows/release.yml` (stage the `.desktop` into the
>   CI `linux-binary` tarball); `packaging/linux/arch/PKGBUILD` (install it in
>   `package()`); `docs/installation.md` (Mode-A: the autostart story).
> **Files NOT touched (owned by sibling Planned/Ready tasks — see §Scope Boundary):**
>   `packaging/linux/aur/PKGBUILD` + `.SRCINFO` + `README.md` (P1.M1.T1.S3);
>   `flake.nix` (P1.M1.T2.S3); `Cargo.toml` `[package.metadata.deb]` +
>   `packaging/debian/*` (P1.M7.T1.S1); `Cargo.toml`
>   `[package.metadata.generate-rpm]` + `packaging/rpm/*` (P1.M7.T2.S1);
>   `packaging/linux/arch/qmkonnect.install` (no hook change needed — the `.desktop`
>   is static); `packaging/linux/systemd/*`, `packaging/linux/udev/*`;
>   the broad docs/Linux-section overhaul incl. the "Hyprland Only" header
>   (P2.M7.T1.S1); `docs/llms_full.txt` (P2.M7.T2.S2); any Rust source;
>   `PRD.md`, `tasks.json`, `.gitignore`.
>
> **What it is:** F17 (PRD §4) = "Universal Linux autostart: an XDG autostart
> `.desktop` alongside the systemd user service, so login-autostart works on
> systemd **and** non-systemd distros." This task creates the `.desktop` source
> file, makes the CI tarball + the Arch source package carry it, and documents the
> service-vs-`.desktop` story. It is **load-bearing on non-systemd distros**
> (MX/Artix/Void/Gentoo) and **belt-and-suspenders on systemd ones** (the service
> stays primary; the `.desktop` is redundant-but-harmless — the single-instance
> dedupe is owned by the tray/runner, not the launcher). See `spec/LINUX.md` §6.3
> + `spec/PACKAGING.md` §4.7 for the authoritative contract.

---

## Goal

**Feature Goal**: Create the F17 XDG autostart entry
(`packaging/linux/xdg/qmkonnect.desktop`) with the **exact contents** mandated by
`spec/PACKAGING.md` §4.7, wire it into the CI `linux-binary` release tarball and
the Arch source `PKGBUILD`, and document the autostart story (systemd user
service vs XDG `.desktop`) in `docs/installation.md`. This is the **prerequisite
source file** that four sibling packaging tasks (AUR `-bin`, Nix, `.deb`, `.rpm`)
consume — they each install this same file to `/etc/xdg/autostart/`.

**Deliverable** (concrete; this task touches ZERO Rust — every artifact is a
static text/YAML/INI file, so validation is `desktop-file-validate` + grep, not
cargo):
- `packaging/linux/xdg/qmkonnect.desktop` — the 10-line `.desktop` (§All Needed
  Context has the verbatim block). `desktop-file-validate` exits clean.
- `.github/workflows/release.yml` — the `linux-binary` "Stage binary tarball"
  step creates `$STAGE/xdg/` and copies the `.desktop` into it, so the AUR `-bin`
  PKGBUILD (P1.M1.T1.S3) can extract+install it.
- `packaging/linux/arch/PKGBUILD` — `package()` installs the `.desktop` to
  `$pkgdir/etc/xdg/autostart/qmkonnect.desktop` (mode 644), mirroring its existing
  `../udev` / `../systemd` install lines.
- `docs/installation.md` — a concise Mode-A subsection under `## Linux` explaining
  the two autostart mechanisms, which wins, the trade-off, and how to disable.

**Success Definition**:
- `packaging/linux/xdg/qmkonnect.desktop` exists and is byte-identical to the
  `spec/PACKAGING.md` §4.7 block (verified by diff/grep).
- `desktop-file-validate packaging/linux/xdg/qmkonnect.desktop` exits 0 with no
  output (the file is spec-compliant).
- The CI tarball staging step + the Arch `PKGBUILD` both reference the new file
  (grep-confirmed), with the staging subdir matching the existing `udev`/`systemd`
  pattern.
- `docs/installation.md` `## Linux` contains a Mode-A autostart paragraph naming
  both mechanisms and the disable method.
- `git diff --stat` lists EXACTLY 4 files (1 created + 3 modified) and touches
  NONE of the sibling-task-owned files in §Scope Boundary.

## User Persona (if applicable)

**Target User**: Linux users who want QMKonnect to **start at login without
manual setup**. Especially: (1) users on **non-systemd distros** (MX, Artix,
Void, Gentoo) where the systemd user-service path doesn't exist — for them this
`.desktop` is the **only** automatic start path; (2) users on systemd distros who
prefer a DE-managed login start over the udev `SYSTEMD_USER_WANTS` hotplug path.

**Use Case**: After `sudo pacman -S qmkonnect` (or `apt install`/`dnf install`),
the user logs in and the DE session manager auto-launches `qmkonnect` from
`/etc/xdg/autostart/qmkonnect.desktop` — no `systemctl --user enable`, no manual
`qmkonnect &`. On systemd distros the udev-started service is still primary;
on non-systemd distros this entry is load-bearing.

**User Journey**: install package → reboot/log in → DE reads
`/etc/xdg/autostart/qmkonnect.desktop` → `Exec=qmkonnect` runs (the app's own
single-instance lock dedupes vs the systemd service on systemd boxes) → tray
icon appears, QMK layer follows focus. To disable: copy the file to
`~/.config/autostart/qmkonnect.desktop` with `Hidden=true`.

**Pain Points Addressed**: (1) "I'm on Artix/Void (no systemd) — there's no
login autostart" → now there is. (2) "Two launchers will double-start it" →
the tray/runner's single-instance lock handles that (the launcher does not).

## Why

- **F17 is an explicit PRD feature** ("Universal Linux autostart … works on
  systemd AND non-systemd distros"). The `.desktop` is the artifact that delivers
  it; without it, non-systemd users have no autostart at all.
- **The spec is already complete and ahead of the implementation.** Both
  `spec/PACKAGING.md` §4.7 and `spec/LINUX.md` §6.3 fully specify the file
  contents, ship path, and the service-vs-`.desktop` trade-off. The shared FHS
  artifact table (`spec/PACKAGING.md` §4) already lists
  `/etc/xdg/autostart/qmkonnect.desktop` as its 7th row. This task is the
  **mechanical realization** of an already-specified contract — an agent that
  "improves" the contents (e.g. absolute `Exec`, dropping the GNOME key, adding
  `TryExec`) would DIVERGE from the human-owned spec. Use the contents verbatim.
- **This task is a prerequisite, not a leaf.** Four sibling packaging tasks
  reference `packaging/linux/xdg/qmkonnect.desktop` in their (spec-mandated)
  asset arrays: the `.deb` `[package.metadata.deb] assets`
  (`["packaging/linux/xdg/qmkonnect.desktop", "etc/xdg/autostart/", "644"]`,
  spec §4.3), the `.rpm` `[package.metadata.generate-rpm] assets`
  (`dest = "/etc/xdg/autostart/qmkonnect.desktop"`, spec §4.4), the AUR `-bin`
  PKGBUILD (installs `${stage}/xdg/qmkonnect.desktop`, P1.M1.T1.S3), and the
  Nix `flake.nix` `postInstall` (P1.M1.T2.S3). If this file is missing or its
  contents drift, ALL FOUR fail their build/install. So: get the file right,
  get it stable, and do not re-touch it later.
- **The trade-off is documented, not "fixed".** The `.desktop` is login-only-start;
  it loses the systemd `BindsTo=dev-qmkonnect_device.device` plug/unplug lifecycle
  (start on plug, stop on unplug). On systemd distros the service stays primary;
  the `.desktop` is redundant-but-harmless. This is the intended design — do not
  try to reconcile the two into one launcher.

## What

1. **`packaging/linux/xdg/qmkonnect.desktop`** — the verbatim `spec/PACKAGING.md`
   §4.7 contents (10 lines incl. a `NoDisplay=true` comment). Ship at
   `/etc/xdg/autostart/qmkonnect.desktop` (mode 644) in every package.
2. **`.github/workflows/release.yml`** — in the `linux-binary` job's "Stage binary
   tarball" step, add `"$STAGE/xdg"` to the `mkdir -p` and add a
   `cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"` line, mirroring the
   existing `udev`/`systemd` subdir staging. The tarball then contains
   `qmkonnect-<ver>-linux-x86_64/xdg/qmkonnect.desktop` for the AUR `-bin` to
   consume.
3. **`packaging/linux/arch/PKGBUILD`** — in `package()`, add a 5th `install -Dm644`
   line installing `../xdg/qmkonnect.desktop` (relative to `$srcdir/..`, matching
   the existing `../udev` / `../systemd` pattern) to
   `$pkgdir/etc/xdg/autostart/qmkonnect.desktop`.
4. **`docs/installation.md`** — a Mode-A subsection under `## Linux` documenting
   the autostart story (see §Implementation Tasks Task 4 for the exact placement
   + wording). Keep it focused; do NOT rewrite the (now-stale) "Hyprland Only"
   header or restructure the section (that is P2.M7.T1.S1).

### Success Criteria
- [ ] `test -f packaging/linux/xdg/qmkonnect.desktop` (file exists).
- [ ] `diff <(sed -n '/^\[Desktop Entry\]/,/^NoDisplay=true/p' spec/PACKAGING.md) \
      packaging/linux/xdg/qmkonnect.desktop` → empty (byte-identical to spec, bar
      the in-spec `# Not shown…` comment which is part of the file).
- [ ] `desktop-file-validate packaging/linux/xdg/qmkonnect.desktop` → exit 0,
      no output.
- [ ] `grep -n 'xdg' .github/workflows/release.yml` → the new `mkdir -p …xdg` +
      `cp …xdg/` lines in the `linux-binary` Stage step.
- [ ] `grep -n 'xdg\|autostart' packaging/linux/arch/PKGBUILD` → the 5th
      `install -Dm644 "../xdg/qmkonnect.desktop" …` line in `package()`.
- [ ] `grep -n 'XDG autostart\|\.desktop\|autostart' docs/installation.md` → the
      new Mode-A autostart subsection present in the `## Linux` block.
- [ ] `git diff --stat` → EXACTLY 4 files; NONE from §Scope Boundary's no-touch list.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can complete this from the PRP + repo
because: (a) the `.desktop` **contents are given verbatim** from
`spec/PACKAGING.md` §4.7 (copied into this PRP below — no guessing); (b) every
edit site is pinpointed to an exact file + the surrounding unique text +
the exact replacement text; (c) the validation gates are non-Rust
(`desktop-file-validate` + `grep`/`diff`), so the parallel `cargo build` redness
(GOTCHA-1) cannot interfere; (d) the scope boundary is explicit (4 sibling tasks)
so the agent cannot collide with parallel/planned work; (e) the research confirmed
`desktop-file-validate` passes clean on the spec contents and that the file's
semantics (`NoDisplay`/`Hidden`/`X-GNOME-Autostart-enabled`) match the spec's
intent — so there is no hidden gotcha in the mandated contents.

### Documentation & References

```yaml
# MUST READ — the authoritative .desktop contents (the WHAT, verbatim).
- file: spec/PACKAGING.md
  why: "§4.7 'XDG autostart .desktop' is the SINGLE source of truth for the file
        contents, ship path (/etc/xdg/autostart/qmkonnect.desktop), and the
        trade-off narrative. §4's shared-FHS-path table lists it as the 7th row.
        §4.6 generic-tarball install.sh shows the install command for the manual
        install. §4.3 (.deb assets) + §4.4 (.rpm assets) show how the sibling
        packages reference the SAME source file (proving it is shared)."
  critical: "Copy the §4.7 INI block VERBATIM into packaging/linux/xdg/qmkonnect.desktop.
             Do NOT 'improve' it (no absolute Exec, no TryExec, no dropped keys) —
             the contents are human-owned; deviating breaks the contract."

# MUST READ — the autostart STORY (the WHY + the Mode-A doc source).
- file: spec/LINUX.md
  why: "§6.3 'XDG autostart .desktop — the universal fallback (F17)' is the
        authoritative narrative for the docs subsection: load-bearing on
        non-systemd distros, belt-and-suspenders on systemd; the trade-off
        (login-only start, loses the BindsTo plug/unplug lifecycle); how to
        disable (Hidden=true copy in ~/.config/autostart/). §6.1 is the systemd
        user service it sits alongside (BindsTo=dev-qmkonnect_device.device)."
  pattern: "The Mode-A doc paragraph should mirror §6.3's three bullet points
            (trade-off / contents / disable) condensed to prose."

# MUST READ — the CI tarball staging site (Task 2 edit target).
- file: .github/workflows/release.yml
  why: "The 'linux-binary' job 'Stage binary tarball' step (L142-184) is the ONLY
        place the release tarball contents are assembled. The existing pattern is:
        `mkdir -p \"$STAGE/udev\" \"$STAGE/systemd\"` then one `cp …\"$STAGE/<dir>/\"`
        per artifact. ADD `\"$STAGE/xdg\"` to the mkdir and one cp line — mirror
        exactly, do not restructure."
  gotcha: "The AUR -bin PKGBUILD (P1.M1.T1.S3) consumes `${stage}/xdg/qmkonnect.desktop`
           from this tarball, so the subdir name MUST be `xdg` (not `autostart`).
           Do NOT touch the AUR PKGBUILD itself (owned by P1.M1.T1.S3) — this task
           only changes the tarball."

# MUST READ — the Arch source PKGBUILD package() (Task 3 edit target).
- file: packaging/linux/arch/PKGBUILD
  why: "`package()` (cd \"$srcdir/..\") installs 4 files via relative paths
        (`../udev/...`, `../systemd/...`). ADD the 5th install of
        `../xdg/qmkonnect.desktop` to `$pkgdir/etc/xdg/autostart/qmkonnect.desktop`
        — mirror the relative-path + `install -Dm644` pattern. The file is STATIC
        (no template substitution), so NO change to qmkonnect.install hooks."
  gotcha: "Do NOT add the .desktop to `backup=(...)` — only the user-instantiated
           service template is preserved across upgrades; the .desktop is
           package-owned (identical on every system). Do NOT add a `depends` for
           a desktop file (there is none)."

# READ — confirm the Arch pacman hooks need NO change (they don't).
- file: packaging/linux/arch/qmkonnect.install
  why: "post_install/post_upgrade/post_remove deal ONLY with the systemd template
        instantiation + udev reload + systemctl --global enable. The .desktop is a
        static file with no lifecycle action, so this file is NOT edited. (Read it
        to CONFIRM — the only .desktop-relevant fact is 'nothing to do here'.)"

# MUST READ — the docs edit target (Task 4).
- file: docs/installation.md
  why: "The `## Linux` section (L97+) is where the Mode-A autostart subsection
        goes. The 'Other Linux Distributions' manual-install block (L115-152)
        currently installs only binary+helper+udev rule (it predates F17). The
        systemd-service step is L126-134. Insert the autostart subsection adjacent
        to the systemd step (both are 'how it starts at login')."
  gotcha: "Do NOT rewrite the 'Linux (Hyprland Only)' header (L99) — the
           cross-DE docs overhaul is P2.M7.T1.S1. Do NOT touch the AUR/Nix/mise
           subsection prose counts ('same paths') — those are P1.M1.T1.S3 /
           P2.M7.T1.S1. This task adds ONE focused subsection (and optionally the
           .desktop install line in the manual block, per spec §4.6)."

# REFERENCE — the consumed sibling PRP (CONTRACT, not edited).
- file: plan/007_fb356ba503b4/P2M5T1S1/PRP.md
  why: "Confirms the parallel task is X11-backend-only (Rust), shares NO files
        with this task (no .desktop/YAML/PKGBUILD overlap), and explains why
        `cargo build --release` is currently RED (private `mod gnome;` reach in
        runners/linux.rs:194) — GOTCHA-1. This task is entirely non-Rust, so that
        redness is irrelevant to its validation."

# EXTERNAL — XDG autostart semantics (validates the spec contents; do NOT use to
# override them).
- url: https://specifications.freedesktop.org/autostart-spec/autostart-spec-latest.html
  why: "§ 'Application Autostart' — /etc/xdg/autostart/ + ~/.config/autostart/
        precedence (per-user same-name OVERRIDES system); the disable mechanism."
- url: https://specifications.freedesktop.org/desktop-entry-spec/latest/
  why: "Desktop Entry Spec — Type/Name/Exec/Icon/Terminal/Categories/NoDisplay
        key semantics. `Hidden=true` = 'entry deleted' (correct disable)."
  critical: "desktop-file-validate (from desktop-file-utils) is the canonical
             syntax checker; research confirmed it passes CLEAN on the spec
             contents (the X- vendor key is not flagged). Use it as Validation
             Gate Level 1."
```

### Current Codebase tree (relevant subset)

```bash
spec/PACKAGING.md                       # §4 + §4.7 (verbatim .desktop contents + ship path) ← READ (source of truth)
spec/LINUX.md                           # §6.3 (the autostart story) + §6.1 (systemd service)  ← READ
.github/workflows/release.yml           # linux-binary job L142-184 (Stage tarball step)        ← EDIT (Task 2)
packaging/linux/arch/PKGBUILD           # package() installs 4 files via ../udev ../systemd     ← EDIT (Task 3)
packaging/linux/arch/qmkonnect.install  # pacman hooks (systemd/udev only)                      ← READ (confirm no change)
packaging/linux/xdg/                    # (DOES NOT EXIST yet)                                  ← CREATE (Task 1)
docs/installation.md                    # ## Linux section L97+                                 ← EDIT (Task 4)
packaging/linux/systemd/qmkonnect.service.template   # the systemd unit (referenced by §6.1) ← READ (context only)
```

### Desired Codebase tree with files added/changed

```bash
packaging/linux/xdg/qmkonnect.desktop   # NEW — the F17 autostart entry (10 lines, verbatim from spec §4.7)
.github/workflows/release.yml           # MODIFIED — Stage step adds $STAGE/xdg + cp .desktop (1 mkdir arg + 1 cp line)
packaging/linux/arch/PKGBUILD           # MODIFIED — package() adds 5th install -Dm644 ../xdg/qmkonnect.desktop
docs/installation.md                    # MODIFIED — +1 Mode-A autostart subsection under ## Linux (+ optional .desktop install line)
```

### Known Gotchas of our codebase & Library Quirks

```text
// CRITICAL (GOTCHA-1 — the main build is RED, but this task builds NOTHING).
//   `cargo build --release` (default features) currently FAILS with
//   `error[E0603]: module \`gnome\` is private` at src/runners/linux.rs:194,
//   caused by the PARALLEL P2.M3.T2.S2 task reaching private `mod gnome;`.
//   That sibling owns the fix. This task is ENTIRELY non-Rust (.desktop / YAML /
//   PKGBUILD / .md), so NO cargo build is part of the validation loop. The only
//   tooling check is `desktop-file-validate` (from `desktop-file-utils`). Do NOT
//   attempt a cargo build to "verify" this task, and do NOT touch any Rust file.

// (GOTCHA-2 — use the spec contents VERBATIM; do not "improve" them).
//   The .desktop contents are human-owned (spec/PACKAGING.md §4.7). Research
//   found that `X-GNOME-Autostart-enabled=true` is REDUNDANT-but-harmless (default
//   is enabled) and that `Exec=qmkonnect` (vs an absolute path) is marginally less
//   robust on minimal WMs. BOTH observations are consistent with the spec's own
//   framing ("redundant-but-harmless") and the package reality (/usr/bin is on
//   $PATH). DO NOT act on them: changing the contents would diverge from the
//   contract and break the 4 sibling packages that expect THIS file. If a
//   refinement is genuinely wanted, it is a SPEC change (human-owned), not this
//   task. Use the §4.7 block verbatim.

// (GOTCHA-3 — wlroots-only compositors don't honor /etc/xdg/autostart natively).
//   GNOME/KDE/XFCE/Cinnamon/MATE/LXQt/Budgie/COSMIC all read /etc/xdg/autostart.
//   Pure wlroots compositors (Sway, Hyprland without a DE) do NOT auto-run it
//   unless the user has `dex` or the systemd `xdg-autostart-generator` active.
//   This does NOT change the file or its contents — it is a DOCUMENTED limitation
//   for the Mode-A docs (the .desktop covers the DE + non-systemd tail; wlroots
//   users on systemd still get the systemd service; wlroots users on non-systemd
//   need dex). Surface it in the docs subsection; do not over-claim "every DE".

// (GOTCHA-4 — the AUR -bin tarball subdir name must be `xdg`).
//   The AUR PKGBUILD (P1.M1.T1.S3, not this task) will install
//   `${stage}/xdg/qmkonnect.desktop`. So the CI tarball MUST stage it at
//   `$STAGE/xdg/qmkonnect.desktop` (not `$STAGE/autostart/` or flat). Mirror the
//   existing `udev`/`systemd` subdir convention. Do NOT touch the AUR PKGBUILD.

// (GOTCHA-5 — the Arch PKGBUILD uses ../../ paths from $srcdir).
//   package() does `cd "$srcdir/.."` then installs `../../../target/...` (built
//   artifacts) and `../udev/...` + `../systemd/...` (repo files). The new .desktop
//   line MUST use the repo-file relative form: `../xdg/qmkonnect.desktop` (so it
//   resolves to packaging/linux/xdg/qmkonnect.desktop from packaging/linux/arch).
//   The `arch` CI job runs makepkg against the full checkout, so the path resolves.

// (GOTCHA-6 — do NOT add the .desktop to PKGBUILD `backup=()`).
//   `backup=` preserves user-edited files across upgrades. Only the
//   user-instantiated service template qualifies. The .desktop is identical on
//   every system (shipped static); over-writing it on upgrade is correct.

// (GOTCHA-7 — NoDisplay vs Hidden, do not confuse in docs).
//   NoDisplay=true (in the shipped file) hides the entry from app menus but
//   leaves autostart ON — correct for an autostart-only entry. Hidden=true (set
//   by the USER in their ~/.config/autostart/ override) DISABLES autostart.
//   The docs must state this distinction precisely (research-confirmed).

// (GOTCHA-8 — do NOT regenerate docs/llms_full.txt).
//   The omnibus doc is owned by P2.M7.T2.S2. This task edits the human-facing
//   docs/installation.md only.
```

## Implementation Blueprint

### Data models and structure

None. No data models. The single artifact is a static `.desktop` INI file whose
contents are fixed by the spec.

### The verbatim `.desktop` contents (from spec/PACKAGING.md §4.7)

```ini
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
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/linux/xdg/qmkonnect.desktop
  - WRITE the file with the EXACT 10-line block above (verbatim from spec/PACKAGING.md §4.7).
  - NAMING: lowercase, kebab-case name `qmkonnect.desktop` (matches the ship path basename
            /etc/xdg/autostart/qmkonnect.desktop and every spec reference).
  - PLACEMENT: packaging/linux/xdg/ (NEW dir; `write` creates it).
  - GOTCHA-2: do NOT alter the contents (no absolute Exec, no TryExec, no dropped
              X-GNOME key) — verbatim only.
  - VERIFY: `desktop-file-validate packaging/linux/xdg/qmkonnect.desktop` → exit 0, no output.
  - VERIFY: `cat packaging/linux/xdg/qmkonnect.desktop` matches the spec block byte-for-byte.

Task 2: MODIFY .github/workflows/release.yml — stage the .desktop in the linux-binary tarball
  - FIND: the `linux-binary` job's `- name: Stage binary tarball` step (run: block).
  - EDIT the `mkdir -p "$STAGE/udev" "$STAGE/systemd"` line → append ` "$STAGE/xdg"`.
    (Result: `mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"`.)
  - ADD, right after the systemd-template cp line
    (`cp packaging/linux/systemd/qmkonnect.service.template "$STAGE/systemd/"`), the line:
      cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"
  - PATTERN: mirror the existing `udev`/`systemd` subdir + cp convention EXACTLY.
  - GOTCHA-4: the subdir name MUST be `xdg` (AUR -bin consumes `${stage}/xdg/...`).
  - VERIFY: `grep -n 'xdg' .github/workflows/release.yml` → both lines in the linux-binary step.
  - NOTE: do NOT touch the AUR PKGBUILD/sha256sums — that re-pin is P1.M1.T1.S3.

Task 3: MODIFY packaging/linux/arch/PKGBUILD — install the .desktop in package()
  - FIND: the `package()` function's last install line (the systemd-template install),
          immediately before the closing `}`.
  - ADD, after the systemd-template install, the 5th install:
      # XDG autostart entry — universal login-autostart fallback (F17; LINUX.md §6.3).
      # Static file (no template instantiation); NoDisplay=true hides it from app menus.
      install -Dm644 "../xdg/qmkonnect.desktop" "$pkgdir/etc/xdg/autostart/qmkonnect.desktop"
  - PATTERN: mirror the existing `install -Dm644 "../<dir>/<file>" "$pkgdir/<dest>"` form
             (GOTCHA-5: the `../xdg/` relative path resolves from packaging/linux/arch).
  - GOTCHA-6: do NOT add the .desktop to `backup=(...)` (only the service template is backed up).
  - GOTCHA: do NOT edit packaging/linux/arch/qmkonnect.install (no hook change — static file).
  - VERIFY: `grep -n 'xdg\|autostart' packaging/linux/arch/PKGBUILD` → the new install line.

Task 4: MODIFY docs/installation.md — Mode-A autostart subsection under ## Linux
  - FIND: the `## Linux` section's "Other Linux Distributions" block, specifically
          right AFTER the systemd-service manual step (the `systemctl --user enable
          --now qmkonnect.service` block, ~L133) and BEFORE step 4 (udev rules).
  - ADD a focused subsection. Suggested heading + prose (condense spec/LINUX.md §6.3):
      ### Autostart at login

      On Linux, QMKonnect can start at login two ways, and the packages set both up:

      - **systemd user service** (primary on systemd distros) — started by the
        static udev rule's `SYSTEMD_USER_WANTS` when your keyboard is present, with
        a `BindsTo` lifecycle that stops/restarts it on unplug/replug. Enable with
        `systemctl --user enable --now qmkonnect.service` (the Arch/AUR/Debian/RPM
        packages enable it globally on install).
      - **XDG autostart entry** (`/etc/xdg/autostart/qmkonnect.desktop`) — a
        universal fallback honored by GNOME, KDE Plasma, XFCE, COSMIC, MATE,
        Cinnamon, LXQt, Budgie and the session-managed tail. It starts the daemon
        at **login on every desktop — systemd or not** (MX, Artix, Void, Gentoo),
        where it is the load-bearing path. On systemd distros it is redundant-but-
        harmless (the daemon's own single-instance lock dedupes the two launches).

      The trade-off: the `.desktop` is login-only-start and loses the systemd
      plug/unplug lifecycle, so on systemd distros the service stays primary. To
      disable the autostart entry, copy it to
      `~/.config/autostart/qmkonnect.desktop` and set `Hidden=true` (the per-user
      copy overrides the system one). Note: pure wlroots compositors (Sway,
      Hyprland without a session manager) do not run `/etc/xdg/autostart` natively
      — install `dex` or enable the systemd `xdg-autostart-generator` there.

  - OPTIONAL (recommended, matches spec/PACKAGING.md §4.6): add the .desktop install
    line to the manual-install snippet in "Other Linux Distributions" step, after the
    udev-rule install:
      sudo install -m644 packaging/linux/xdg/qmkonnect.desktop /etc/xdg/autostart/
    (Only if the snippet exists in that form — do not invent a new block.)
  - GOTCHA-3: include the wlroots/dex caveat (do NOT claim "every DE" runs it).
  - GOTCHA-7: state NoDisplay vs Hidden precisely (NoDisplay hides menus; Hidden disables).
  - GOTCHA: do NOT rewrite the "Linux (Hyprland Only)" header or AUR/Nix prose counts
            (owned by P2.M7.T1.S1 / P1.M1.T1.S3).
  - VERIFY: `grep -n 'XDG autostart\|\.desktop\|autostart' docs/installation.md` → the new subsection.
```

### Implementation Patterns & Key Details

```bash
# ── The single source-of-truth comparison (validate the created file is verbatim) ─
# spec/PACKAGING.md §4.7 INI block ⟷ packaging/linux/xdg/qmkonnect.desktop
# These must match byte-for-byte. desktop-file-validate is the syntax gate.

# ── The CI tarball staging pattern (release.yml linux-binary job) ────────────────
# EXISTING:
#   mkdir -p "$STAGE/udev" "$STAGE/systemd"
#   cp packaging/linux/udev/69-qmkonnect-rawhid.rules          "$STAGE/udev/"
#   cp packaging/linux/systemd/qmkonnect.service.template      "$STAGE/systemd/"
# ADD (mirror the subdir + cp convention):
#   mkdir -p "$STAGE/udev" "$STAGE/systemd" "$STAGE/xdg"
#   cp packaging/linux/xdg/qmkonnect.desktop                   "$STAGE/xdg/"

# ── The Arch PKGBUILD package() install pattern ─────────────────────────────────
# EXISTING (relative to $srcdir/..):
#   install -Dm644 "../udev/69-qmkonnect-rawhid.rules"        "$pkgdir/usr/lib/udev/rules.d/..."
#   install -Dm644 "../systemd/qmkonnect.service.template"    "$pkgdir/usr/lib/systemd/user/..."
# ADD (same relative-path + install -Dm644 form):
#   install -Dm644 "../xdg/qmkonnect.desktop"                 "$pkgdir/etc/xdg/autostart/qmkonnect.desktop"
```

### Integration Points

```yaml
CI (release.yml linux-binary job):
  - tarball gains qmkonnect-<ver>-linux-x86_64/xdg/qmkonnect.desktop.
  - consumed downstream by: AUR -bin PKGBUILD (P1.M1.T1.S3 — re-pins sha256sums).

ARCH SOURCE PACKAGE (packaging/linux/arch/PKGBUILD):
  - package() installs the 5th file to etc/xdg/autostart/ (644).
  - qmkonnect.install hooks: NO change (static file, no lifecycle action).
  - the `arch` CI job (makepkg against full checkout) resolves ../xdg/qmkonnect.desktop.

DOCS (docs/installation.md):
  - +1 Mode-A subsection (systemd service vs XDG .desktop) under ## Linux.
  - optional: +1 .desktop install line in the manual-install snippet (spec §4.6).

DOWNSTREAM CONSUMERS (NOT this task — listed for handoff clarity):
  - AUR -bin PKGBUILD package(): install ${stage}/xdg/qmkonnect.desktop  (P1.M1.T1.S3)
  - Nix flake.nix postInstall: install the .desktop                      (P1.M1.T2.S3)
  - [package.metadata.deb] assets: already refs this file in spec §4.3   (P1.M7.T1.S1)
  - [package.metadata.generate-rpm] assets: already refs it in spec §4.4 (P1.M7.T2.S1)
  note: "Cargo.toml has NEITHER metadata block today; P1.M7.T1/T2.S1 create them,
         both pointing at packaging/linux/xdg/qmkonnect.desktop — which Task 1 supplies."
```

## Validation Loop

> This task is entirely non-Rust. **Do NOT run `cargo build`** to validate it
> (the default-features build is RED from parallel work — GOTCHA-1 — and is
> irrelevant here). The gates below are all static-file checks.

### Level 1: `.desktop` syntax (the headline gate)

```bash
# Requires desktop-file-utils (Arch: pacman -S desktop-file-utils; Debian: apt install
# desktop-file-utils; Fedora: dnf install desktop-file-utils). Research confirmed the
# spec contents pass CLEAN (no warnings; the X- vendor key is not flagged).
desktop-file-validate packaging/linux/xdg/qmkonnect.desktop
# Expected: exit 0, NO output. If it prints anything, the file diverged from the spec —
# re-copy the §4.7 block verbatim (GOTCHA-2) and re-run.

# Byte-identical to the spec source-of-truth:
diff <(sed -n '/^\[Desktop Entry\]/,/^NoDisplay=true/p' spec/PACKAGING.md) \
     packaging/linux/xdg/qmkonnect.desktop
# Expected: empty diff. (The spec block includes the trailing `# Not shown…` comment +
# `NoDisplay=true`; the sed range captures exactly the shipped file.)
```

### Level 2: File presence + wiring (grep gates)

```bash
# Task 1 — file exists:
test -f packaging/linux/xdg/qmkonnect.desktop && echo OK
# Expected: OK

# Task 2 — CI tarball staging:
grep -n '\$STAGE/xdg\|xdg/qmkonnect.desktop' .github/workflows/release.yml
# Expected: the `mkdir -p … "$STAGE/xdg"` line AND the `cp packaging/linux/xdg/qmkonnect.desktop "$STAGE/xdg/"` line,
# both inside the linux-binary job's 'Stage binary tarball' step.

# Task 3 — Arch PKGBUILD install:
grep -n 'xdg/qmkonnect.desktop\|etc/xdg/autostart' packaging/linux/arch/PKGBUILD
# Expected: the `install -Dm644 "../xdg/qmkonnect.desktop" "$pkgdir/etc/xdg/autostart/qmkonnect.desktop"` line in package().

# Task 3 (negative) — hooks UNCHANGED:
grep -c 'desktop\|xdg' packaging/linux/arch/qmkonnect.install
# Expected: 0 (confirm no spurious hook edit; the .desktop needs no lifecycle action).

# Task 4 — docs subsection:
grep -n 'XDG autostart\|/etc/xdg/autostart\|Hidden=true' docs/installation.md
# Expected: ≥1 hit in the new ## Linux subsection.
```

### Level 3: Structural sanity (the Arch package builds the .desktop)

```bash
# Confirm the PKGBUILD's relative path resolves to the repo file (the `arch` CI job
# runs makepkg against the full checkout from packaging/linux/arch, so ../xdg → packaging/linux/xdg):
test -f packaging/linux/xdg/qmkonnect.desktop && \
  echo "../xdg/qmkonnect.desktop resolves to $(realpath packaging/linux/xdg/qmkonnect.desktop)"
# Expected: resolves to .../packaging/linux/xdg/qmkonnect.desktop

# (OPTIONAL, only if desktop-file-utils + makepkg are available locally and you want a
# full local build — NOT required; CI does this in the `arch` job.)
# cd packaging/linux/arch && makepkg -f 2>&1 | tail -5   # then:
# bsdtar -tf qmkonnect-*.pkg.tar.zst | grep 'etc/xdg/autostart'
# Expected: etc/xdg/autostart/qmkonnect.desktop is listed in the built .pkg.tar.zst.
```

### Level 4: Scope-boundary guard (catch stray edits to sibling-owned files)

```bash
git diff --stat
# Expected: EXACTLY these 4 files (1 created + 3 modified):
#   packaging/linux/xdg/qmkonnect.desktop   (new)
#   .github/workflows/release.yml
#   packaging/linux/arch/PKGBUILD
#   docs/installation.md

# Negative — NOTHING from the sibling-owned set changed:
git diff --stat -- \
  packaging/linux/aur/PKGBUILD packaging/linux/aur/.SRCINFO packaging/linux/aur/README.md \
  flake.nix Cargo.toml packaging/debian packaging/rpm \
  packaging/linux/arch/qmkonnect.install packaging/linux/systemd packaging/linux/udev \
  docs/llms_full.txt src PRD.md
# Expected: EMPTY.
```

## Final Validation Checklist

### Technical Validation
- [ ] `desktop-file-validate packaging/linux/xdg/qmkonnect.desktop` → exit 0, no output.
- [ ] `diff <(sed … spec/PACKAGING.md) packaging/linux/xdg/qmkonnect.desktop` → empty.
- [ ] All Level 2 grep gates pass (file exists; CI staging lines present; PKGBUILD
      install line present; hooks UNCHANGED; docs subsection present).
- [ ] `git diff --stat` → EXACTLY 4 files; the Level 4 negative-diff is EMPTY.

### Feature (contract) Validation
- [ ] `.desktop` contents are byte-identical to `spec/PACKAGING.md` §4.7 (verbatim).
- [ ] Ship path is `/etc/xdg/autostart/qmkonnect.desktop` in the PKGBUILD (644).
- [ ] The CI tarball stages `$STAGE/xdg/qmkonnect.desktop` (subdir name `xdg`).
- [ ] Docs subsection names BOTH mechanisms (systemd service + XDG .desktop), the
      trade-off (login-only start; loses BindsTo lifecycle), and the disable method
      (`Hidden=true` in `~/.config/autostart/`), plus the wlroots/dex caveat.

### Scope & Code-Quality Validation
- [ ] NO edits to `packaging/linux/aur/*`, `flake.nix`, `Cargo.toml` metadata blocks,
      `packaging/debian/*`, `packaging/rpm/*` (sibling tasks).
- [ ] NO edit to `packaging/linux/arch/qmkonnect.install` (static file needs no hook).
- [ ] NO Rust edits; NO `cargo build` run for this task (GOTCHA-1).
- [ ] NO regeneration of `docs/llms_full.txt` (GOTCHA-8).
- [ ] `.desktop` NOT added to PKGBUILD `backup=()` (GOTCHA-6).

### Documentation & Deployment
- [ ] The Mode-A autostart subsection is self-contained (a reader knows both start
      paths + how to disable without reading the spec).
- [ ] No over-claim: the wlroots-without-session-manager limitation is stated.

---

## Anti-Patterns to Avoid

- ❌ Don't "improve" the `.desktop` contents (absolute `Exec`, `TryExec`, dropped
  `X-GNOME-Autostart-enabled`) — they are human-owned (spec §4.7); use them
  verbatim. Research flags are observations, not license to diverge (GOTCHA-2).
- ❌ Don't run `cargo build` to validate this task — it's 100% non-Rust and the
  default-features build is RED from parallel work anyway (GOTCHA-1).
- ❌ Don't edit the AUR `-bin` PKGBUILD / `.SRCINFO` / `README.md`, the Nix
  `flake.nix`, or the `.deb`/`.rpm` Cargo metadata — those are sibling tasks
  (P1.M1.T1.S3 / P1.M1.T2.S3 / P1.M7.T1.S1 / P1.M7.T2.S1). This task only
  creates the source file + wires the CI tarball + Arch PKGBUILD + docs.
- ❌ Don't change `packaging/linux/arch/qmkonnect.install` — the `.desktop` is a
  static file with no instantiation/reload lifecycle (nothing for the hooks to do).
- ❌ Don't add the `.desktop` to PKGBUILD `backup=()` — only the user-instantiated
  service template is preserved across upgrades (GOTCHA-6).
- ❌ Don't stage the tarball copy under `$STAGE/autostart/` or flat — the subdir
  MUST be `xdg` (the AUR `-bin` PKGBUILD consumes `${stage}/xdg/…`; GOTCHA-4).
- ❌ Don't claim "every DE" runs the `.desktop` — wlroots-only compositors need
  `dex`/`xdg-autostart-generator` (GOTCHA-3). State the limitation in docs.
- ❌ Don't confuse `NoDisplay=true` (hide from menus; autostart ON) with
  `Hidden=true` (disable autostart) in the docs (GOTCHA-7).
- ❌ Don't rewrite the "Linux (Hyprland Only)" header or AUR/Nix prose counts —
  the broad docs overhaul is P2.M7.T1.S1 / P1.M1.T1.S3.

---

**Confidence Score: 9/10** for one-pass completion. The deliverable is a
deterministic, spec-pinned static file plus three pinpointed mechanical edits
(tarball staging, PKGBUILD install, docs subsection), all non-Rust, so the
parallel `cargo build` redness cannot interfere. `desktop-file-validate` is a
fast, unambiguous gate that research confirmed passes clean on the spec contents.
The one residual risk is an agent "improving" the human-owned contents or
straying into a sibling task's files — §Known Gotchas #1/#2 + the Level-4
scope-guard + §Anti-Patterns guard against both explicitly.