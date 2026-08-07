# PRP — P2.M7.T1.S1: Update installation/platforms/troubleshooting docs for F16/F17

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Files MODIFIED (core, all markdown):** `README.md`, `docs/installation.md`,
>   `docs/troubleshooting.md`.
> **Files MODIFIED (optional, adjacent consistency):** `docs/configuration.md`
>   (two `(Hyprland only)` annotations → clarify they refer to the Hyprland IPC
>   backend specifically, not Linux overall). Low-risk, unowned by any sibling.
> **Files NOT touched (see §Scope Boundary):** `spec/*.md` (human-owned, read-only
>   reference); `docs/llms_full.txt` (owned by P2.M7.T2.S2 — the omnibus regen);
>   the `### Autostart at login` subsection of `docs/installation.md` (owned by
>   the parallel P2.M6.T1.S1 — PRESERVE, do not move or rewrite); `PRD.md`,
>   `tasks.json`, `prd_snapshot.md`, `.gitignore`; any Rust source; any
>   `packaging/*`; any `.github/workflows/*` (CI jobs are P2.M7.T2.S1).
>
> **What it is:** F16 (PRD §4) = "Cross-DE Linux window monitor: runtime-selected
> backend — foreign-toplevel Wayland → GNOME (Shell extension over D-Bus) →
> Hyprland IPC → AT-SPI → X11." F17 (PRD §4) = "Universal Linux autostart."
> Every P2.M1–P2.M6 deliverable is now complete/in-progress (the five backends
> exist in `src/platforms/`, `select_linux_backend` probes them at runtime, the
> `[linux]` config table + XDG `.desktop` exist). **This task updates the
> user-facing docs so they stop saying "Hyprland only" and instead document the
> real cross-DE coverage, backend auto-selection, the user-installed GNOME
> extension, the GNOME tray (AppIndicator) story, AT-SPI best-effort caveats +
> a11y-enable requirement, and F17 autostart.** It is the documentation
> counterpart to the F16/F17 implementation — pure markdown, no code, no specs.

---

## Goal

**Feature Goal**: Replace every stale "Linux (Hyprland only)" / "other window
managers are not supported yet" claim in `README.md`, `docs/installation.md`, and
`docs/troubleshooting.md` with the truthful cross-DE story from `spec/PLATFORMS.md`
§6–§10 + `spec/LINUX.md` §6–§7, so a Linux user on GNOME / KDE Plasma / COSMIC /
Hyprland / Sway / Niri / wlroots family / XFCE / MATE / Cinnamon / Budgie / LXQt
understands (a) QMKonnect supports their desktop, (b) the backend is chosen
automatically at runtime (`qmkonnect -v` shows which), (c) GNOME needs the
user-installed Shell extension for windows and the AppIndicator extension for the
tray, (d) AT-SPI is a best-effort fallback that needs accessibility enabled, and
(e) login-autostart works on systemd **and** non-systemd distros (F17).

**Deliverable** (concrete; 100% markdown — validation is grep + link checks, not cargo):
- **`README.md`** — Features "Linux" line + the Linux Installation block + the
  Package Managers table reflect broad DE coverage + GNOME extension note + the
  `.deb`/`.rpm` channels.
- **`docs/installation.md`** — the intro compatibility matrix Linux row + the
  `## Linux` section: rewrite the "Linux (Hyprland Only)" header into a cross-DE
  intro with the backend-selection table, explain `qmkonnect -v` + `[linux]
  backend`, expand the GNOME-extension install steps, add `.deb`/`.rpm` to
  Package Managers, and ensure the narrative is consistent with (without
  duplicating) the `### Autostart at login` subsection P2.M6.T1.S1 adds.
- **`docs/troubleshooting.md`** — replace the "Linux (Hyprland Only)" Window-
  Detection subsection with a backend-selection diagnosis block; **remove both**
  "Only Hyprland is supported" notes; **add** three new entries: "No tray icon on
  GNOME" (AppIndicator extension), "AT-SPI best-effort caveats + enable
  accessibility", and "Wrong / inconsistent window names on GNOME" (extension
  missing → AT-SPI fallback).

**Success Definition**:
- `grep -rin "hyprland only\|only supports hyprland\|not supported yet\|other window managers are not supported\|please contribute support for your window manager"` over `README.md docs/installation.md docs/troubleshooting.md` → **ZERO hits** (the stale claims are gone).
- `docs/installation.md` `## Linux` section names the supported desktops (GNOME / KDE Plasma / COSMIC / Hyprland / Sway / Niri / wlroots / XFCE / MATE / Cinnamon / Budgie / LXQt), the backend-selection priority, `qmkonnect -v`, and the `[linux] backend` override.
- `docs/troubleshooting.md` has working anchors for "no tray icon on GNOME",
  "AT-SPI best-effort", and "enable accessibility", each citing the right spec
  section + (for GNOME) the AppIndicator EGO page.
- `README.md` Package Managers table includes `.deb` / `.rpm` rows.
- The P2.M6.T1.S1 `### Autostart at login` subsection is **untouched** (grep
  confirms it still contains the F17 XDG-`.desktop` prose).
- `git diff --stat` lists only markdown files (`.md`) — **no** `spec/`, no Rust,
  no `packaging/`, no `docs/llms_full.txt`, no `PRD.md`/`tasks.json`.

## User Persona (if applicable)

**Target User**: a Linux user who is **not** on Hyprland — a GNOME, KDE Plasma,
COSMIC, Sway, Niri, XFCE, or Cinnamon user who today reads the README, sees
"Linux: Arch/Hyprland only", and assumes QMKonnect won't work for them. Secondary:
a Hyprland user who needs the backend-selection troubleshooting path; a
non-systemd distro user (MX/Artix/Void/Gentoo) who needs to know autostart works.

**Use Case**: A GNOME user finds QMKonnect, installs it, and sees no window
detection and no tray icon. They open troubleshooting, find the exact two fixes
(install the `qmkonnect@mulletware` Shell extension for windows; install the
AppIndicator extension for the tray), and the `qmkonnect -v` command that confirms
which backend was selected and why.

**User Journey**: read README → see their DE listed as supported → install
(package manager or binary) → run `qmkonnect -v` to confirm backend → (GNOME
only) install the Shell extension + optionally AppIndicator → tray icon appears,
layers follow focus → (non-systemd) it auto-starts at login via the XDG
`.desktop`.

**Pain Points Addressed**: (1) "It says Hyprland only, so I didn't try it" → the
docs now list their desktop. (2) "No tray icon on GNOME" → documented AppIndicator
fix. (3) "GNOME shows wrong/empty app names" → documented as the AT-SPI fallback
+ the extension fix. (4) "How do I know which backend is running?" → `qmkonnect -v`.

## Why

- **The docs contradict the shipped product.** F16 (P2.M1–P2.M5) is complete: all
  five backends (`wayland_ft.rs`, `gnome.rs`, `hyprland.rs`, `atspi.rs`, `x11.rs`)
  exist and `select_linux_backend` chooses at runtime. F17 (P2.M6) ships the XDG
  `.desktop`. Yet `README.md:21` still says "Linux: Arch/Hyprland only" and
  `docs/installation.md:99` still has a `### Linux (Hyprland Only)` header telling
  users "Other window managers are not supported yet." This is a correctness bug
  in the docs that suppresses adoption on every non-Hyprland desktop.
- **The specs are authoritative and ahead of the docs.** `spec/PLATFORMS.md` §6–§10
  and `spec/LINUX.md` §6–§7 fully specify the backend matrix, the GNOME two-
  problem split (window extension vs tray AppIndicator), the AT-SPI caveats, and
  the autostart story. The PRD §5 compatibility matrix already claims the broad
  coverage. This task is the **mechanical propagation** of that spec truth into
  the human-facing docs — an agent that "hedges" (e.g. keeping "Hyprland only" as
  a qualifier) would re-introduce the contradiction. State the coverage plainly.
- **GNOME has two genuinely-separate gotchas** that users WILL hit and that the
  current docs do not cover together: (a) no window detection without the
  user-installed Shell extension; (b) no tray icon without the AppIndicator
  extension. Both are solved in code (PLATFORMS.md §8, LINUX.md §7.4) but
  undocumented in troubleshooting → the #1 source of "it doesn't work on GNOME"
  reports. This task documents both with the exact fix.
- **AT-SPI is best-effort and needs accessibility enabled** — without docs, a
  GNOME-without-extension user gets inconsistent/empty app names (the AT-SPI
  fallback) and has no idea why or how to fix it (enable a11y / install the
  extension). PLATFORMS.md §9 spells out the limitations; this task surfaces them.

## What

The full set of edits, grouped by file. **Exact stale strings + line numbers are
in §"All Needed Context → Current Codebase tree".** All replacements use the
authoritative backend matrix from `spec/PLATFORMS.md` §6 (reproduced in this PRP
under Implementation Blueprint). All GNOME/AppIndicator/AT-SPI wording uses the
exact strings from `spec/LINUX.md` §7.4 + `spec/PLATFORMS.md` §8–§9.

1. **`README.md`** —
   - Features → "Linux: Arch/Hyprland only" → broad DE list (one line).
   - Linux Installation: broaden the "### Arch Linux" + "### Other Linux Systems"
     framing to cover the DEs + the GNOME extension note + `.deb`/`.rpm`.
   - Package Managers table: add `.deb` (Debian/Ubuntu) and `.rpm` (Fedora) rows.
2. **`docs/installation.md`** —
   - Intro compatibility table: Linux row → supported DEs + `.deb`/`.rpm` + XDG autostart.
   - `## Linux` section: rewrite `### Linux (Hyprland Only)` → `### Linux` with a
     cross-DE intro + a compact backend-selection table + `qmkonnect -v` + `[linux]
     backend` override. **Preserve** the existing `### GNOME (optional Shell
     extension)` section and the P2.M6.T1.S1 `### Autostart at login` subsection.
   - Package Managers: add `.deb` (cargo-deb) + `.rpm` (cargo-generate-rpm) blocks.
3. **`docs/troubleshooting.md`** —
   - Replace `#### Linux (Hyprland Only)` (Window Detection) with a `#### Linux`
     backend-selection diagnosis (run `qmkonnect -v`, read the selected backend,
     per-backend hints) — keep the Hyprland socket commands as the Hyprland-IPC
     diagnostic, drop the "Only Hyprland is supported" note.
   - Remove the second "Only Hyprland is supported" note under
     `#### Hyprland Integration Issues`.
   - **Add** (under `### Linux Issues`): `#### No tray icon on GNOME` (AppIndicator
     extension), `#### AT-SPI best-effort / enable accessibility`, and (under Window
     Detection) a `#### Linux (GNOME)` entry for "wrong/inconsistent app names"
     (extension missing → AT-SPI fallback).
4. **`docs/configuration.md`** *(optional, adjacent)* — clarify the two
   `(Hyprland only)` annotations (`poll_interval_ms`) to "(Hyprland IPC backend)"
   so they don't read as "Linux = Hyprland only".

### Success Criteria
- [ ] Zero stale "Hyprland only" / "not supported yet" strings in the 3 core files.
- [ ] `docs/installation.md` `## Linux` lists all supported desktops + the backend-selection table + `qmkonnect -v` + `[linux] backend`.
- [ ] `docs/troubleshooting.md` has the three new Linux entries (tray/AppIndicator, AT-SPI best-effort + a11y-enable, GNOME wrong-names).
- [ ] `README.md` Package Managers table includes `.deb` and `.rpm`.
- [ ] P2.M6.T1.S1's `### Autostart at login` subsection is byte-for-byte intact.
- [ ] `git diff --stat` shows only `.md` files; no `spec/`, Rust, `packaging/`, `docs/llms_full.txt`, `PRD.md`/`tasks.json`.

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can complete this from the PRP + repo
because: (a) every stale string is given with its **exact current text + line
number** (grep-verified) so the edits are unambiguous; (b) the authoritative
backend matrix is reproduced **verbatim** in this PRP (from `spec/PLATFORMS.md`
§6) — no guessing at coverage or probe order; (c) the GNOME two-problem split
(window extension vs tray AppIndicator) and the AT-SPI caveats are spelled out
with the exact spec section + external URL + exact command to cite; (d) the scope
boundary with the parallel P2.M6.T1.S1 task is explicit (preserve its autostart
subsection); (e) validation is non-Rust (grep + link checks), so the parallel
`cargo build` redness cannot interfere. The only judgment call is wording — and
the PRP gives ready-to-paste prose for every non-trivial block.

### Documentation & References

```yaml
# MUST READ — the authoritative backend matrix (the WHAT for installation.md + README).
- file: spec/PLATFORMS.md
  why: "§6 'Linux Backend Selection (select_linux_backend)' is the SINGLE source of
        truth for the priority table (foreign-toplevel → GNOME → Hyprland → AT-SPI →
        X11), the availability probes, the config override ([linux] backend), the
        verbose logging, and the no-backend fallback. §7.2 is the compositor coverage
        table for foreign-toplevel (Hyprland/Sway/Niri/River/Labwc/Wayfire/KDE Plasma
        6/COSMIC; GNOME=❌). §8 is the GNOME Shell-extension backend + D-Bus contract
        + first-run notification. §9 is the AT-SPI fallback + its best-effort
        limitations + the a11y-enable requirement. §10 is X11 (gated on
        $WAYLAND_DISPLAY unset)."
  critical: "Reproduce the §6 priority table in installation.md (condensed). State
             coverage PLAINLY (the specs already do) — do NOT hedge with 'Hyprland
             only'. The verbose log line format: 'select_linux_backend: probing X…'
             (src/platforms/linux.rs:143)."

# MUST READ — the GNOME tray story (the WHAT for troubleshooting 'no tray icon').
- file: spec/LINUX.md
  why: "§7.4 'GNOME: the SNI holdout (AppIndicator)' is the authoritative text for
        the 'no tray icon on GNOME' troubleshooting entry: stock GNOME dropped
        SNI/AppIndicator → ksni item invisible → fix = install the 'AppIndicator and
        KStatusNotifierItem Support' extension, OR run trayless (CLI flags). §6.3 is
        the F17 autostart narrative (owned by P2.M6.T1.S1 — reference, don't
        duplicate). §6.1 is the systemd user service."
  critical: "The two GNOME problems (window extension vs tray AppIndicator) are
             SEPARATE — do not conflate them in the docs. Do NOT suggest building a
             GNOME-native tray (the spec explicitly says not to)."

# MUST READ — the PRD compatibility matrix (the source the docs must match).
- file: plan/007_fb356ba503b4/prd_snapshot.md
  why: "§5 'Supported Platforms (Compatibility Matrix)' Linux row already claims the
        broad coverage: 'GNOME / KDE Plasma / COSMIC / Hyprland / Sway / Niri /
        wlroots (Wayland); XFCE / MATE / Cinnamon / Budgie / LXQt (X11)' and lists the
        channels 'AUR · Nix flake · .deb/.rpm · PKGBUILD/binary' + autostart
        'systemd BindsTo device + XDG autostart .desktop'. The README/installation
        Linux rows must mirror this exactly."
  critical: "This is the human-approved matrix — match it verbatim. The '(GNOME:
             AppIndicator extension)' note in the Notes column is the tray caveat."

# MUST READ — the parallel autostart PRP (CONTRACT — do not duplicate its doc edit).
- file: plan/007_fb356ba503b4/P2M6T1S1/PRP.md
  why: "P2.M6.T1.S1 is IN PROGRESS and EDITS docs/installation.md: it adds a
        '### Autostart at login' subsection under ## Linux (systemd service vs XDG
        .desktop, trade-off, disable via Hidden=true, wlroots/dex caveat). This task
        MUST treat that subsection as a fixed existing anchor — preserve it, do not
        move/rewrite it, do not re-explain F17 autostart in detail."
  critical: "If both tasks edit installation.md, anchor your edits on stable text
             (the '### Linux (Hyprland Only)' header, the intro table, the Package
             Managers block) and leave the autostart subsection's text untouched."

# EXTERNAL — citable URLs for the user-facing troubleshooting/install docs.
- url: https://extensions.gnome.org/extension/615/appindicator-support/
  why: "The canonical 'AppIndicator and KStatusNotifierItem Support' GNOME extension
        page — the fix for 'no tray icon on GNOME'. Cite it (and the distro pkg
        'gnome-shell-extension-appindicator' on Arch/Debian/Fedora)."
- url: https://github.com/ubuntu/gnome-shell-extension-appindicator
  why: "Upstream repo for the AppIndicator extension (for the install-from-source
        fallback / shell-version support)."
- url: https://www.freedesktop.org/wiki/Accessibility/AT-SPI2/
  why: "AT-SPI2 — confirms 'gsettings set org.gnome.desktop.interface
        toolkit-accessibility true' enables the a11y bus (required for the AT-SPI
        backend). Cite in the 'enable accessibility' troubleshooting step."
- url: https://wayland.app/protocols/wlr-foreign-toplevel-management-unstable-v1
  why: "The wlr-foreign-toplevel protocol spec — explains which compositors
        implement it (the foreign-toplevel backend's coverage). Optional citation."
- url: https://specifications.freedesktop.org/autostart-spec/autostart-spec-latest.html
  why: "XDG autostart spec — NoDisplay vs Hidden semantics; per-user override of
        /etc/xdg/autostart. (Already cited by P2.M6.T1.S1; reference, don't redo.)"

# READ — the three files being edited (their current full text).
- file: README.md
  why: "Features L21 ('Linux: Arch/Hyprland only'); Installation '### Arch Linux' +
        '### Other Linux Systems'; Package Managers table (no .deb/.rpm). The edit
        sites are these three blocks."
- file: docs/installation.md
  why: "Intro compat table L27 (Linux row); '### Linux (Hyprland Only)' L99 + note
        L101; '### GNOME (optional Shell extension)' L191 (keep, reframe); '### Package
        Managers' L289 (add .deb/.rpm). The P2.M6.T1.S1 autostart subsection lands
        near the systemd step (~L133) — preserve it."
- file: docs/troubleshooting.md
  why: "'#### Linux (Hyprland Only)' L203 + note L214 (under Window Detection →
        rewrite); '#### Linux (GNOME)' L216 (keep — it's the window-extension diag);
        '### Linux Issues' L319; '#### Hyprland Integration Issues' L382 + note L402
        (drop the note). New entries go under '### Linux Issues'."
```

### Current Codebase tree (the exact stale strings + edit sites — grep-verified)

```bash
# README.md
README.md:21:  - Linux: Arch/Hyprland only                      # ← REWRITE (Features)
README.md:71:### Arch Linux                                     # ← BROADEN (Installation)
README.md:84:### Other Linux Systems                            # ← BROADEN (Installation)
README.md (Package Managers table): Linux rows = AUR/Nix only   # ← ADD .deb/.rpm rows

# docs/installation.md
docs/installation.md:27:| **Linux** (Hyprland) | binary / Arch PKGBUILD | AUR · Nix |   # ← REWRITE (intro compat table)
docs/installation.md:99:### Linux (Hyprland Only)                                    # ← REWRITE → "### Linux"
docs/installation.md:101:**Note**: QMKonnect currently only supports Hyprland ...      # ← DELETE (replaced by cross-DE intro)
docs/installation.md:103:#### Arch Linux                                             # ← KEEP (broaden framing)
docs/installation.md:115:#### Other Linux Distributions                              # ← KEEP
docs/installation.md:191:### GNOME (optional Shell extension)                        # ← KEEP (already correct — reframe under new header)
docs/installation.md:289:### Package Managers                                        # ← ADD .deb/.rpm blocks
# (P2.M6.T1.S1 adds "### Autostart at login" near the systemd step ~L133 — PRESERVE)

# docs/troubleshooting.md
docs/troubleshooting.md:203:#### Linux (Hyprland Only)            # ← REWRITE → "#### Linux" (backend-selection diagnosis)
docs/troubleshooting.md:214:**Note**: Only Hyprland is supported … # ← DELETE
docs/troubleshooting.md:216:#### Linux (GNOME)                    # ← KEEP (window-extension diag, already good)
docs/troubleshooting.md:319:### Linux Issues                      # ← ADD 3 new subsections under here
docs/troubleshooting.md:382:#### Hyprland Integration Issues      # ← KEEP (drop the note at L402)
docs/troubleshooting.md:402:**Note**: Only Hyprland is supported … # ← DELETE

# docs/configuration.md (OPTIONAL adjacent edit)
docs/configuration.md:74:# (Hyprland only) periodic active-window poll interval (ms).  # → "(Hyprland IPC backend)"
docs/configuration.md:221:| `poll_interval_ms` | `0` | (Hyprland only) … |              # → "(Hyprland IPC backend)"
```

### Desired Codebase tree with files changed

```bash
README.md               # MODIFIED — Features Linux line, Linux install framing, Package Managers table (+.deb/.rpm)
docs/installation.md    # MODIFIED — intro compat table Linux row, "## Linux" cross-DE intro + backend table + qmkonnect -v + [linux] backend, GNOME section reframed, .deb/.rpm package blocks; P2.M6.T1.S1 autostart subsection PRESERVED
docs/troubleshooting.md # MODIFIED — "#### Linux" backend-selection diag (replaces Hyprland-Only); 2 stale notes removed; +3 new Linux entries (AppIndicator tray, AT-SPI best-effort + a11y-enable, GNOME wrong-names)
docs/configuration.md   # OPTIONAL — 2 "(Hyprland only)" → "(Hyprland IPC backend)" clarifications
```

### The authoritative backend matrix (reproduce in installation.md — from spec/PLATFORMS.md §6)

```
select_linux_backend probes in PRIORITY ORDER (first available wins; all compiled
in by default; qmkonnect -v prints each probe + the choice):

| # | Backend             | Covers                                           | Availability probe                                            |
|---|---------------------|--------------------------------------------------|---------------------------------------------------------------|
| 1 | foreign-toplevel    | Hyprland, Sway, Niri, River, Labwc, Wayfire,     | $WAYLAND_DISPLAY + wlr-foreign-toplevel global                |
|   | (Wayland)           | KDE Plasma 6, COSMIC                             |                                                               |
| 2 | GNOME (extension)   | GNOME (Mutter — no foreign-toplevel)             | D-Bus name io.mulletware.QMKonnect owned (extension enabled)  |
| 3 | Hyprland (IPC)      | Hyprland (legacy fallback; #1 supersedes)        | $HYPRLAND_INSTANCE_SIGNATURE + socket                         |
| 4 | AT-SPI (a11y bus)   | any compositor with a11y ON (best-effort)        | org.a11y.Bus owned / $ATSPI_BUS_ADDRESS                       |
| 5 | X11                 | XFCE, MATE, Cinnamon, Budgie, LXQt               | $DISPLAY set AND $WAYLAND_DISPLAY unset AND xprop present     |

Override: [linux] backend = "foreign-toplevel"|"gnome"|"hyprland"|"atspi"|"x11"|"auto" (default auto).
No backend found (e.g. GNOME-Wayland, extension uninstalled, a11y off) → tray + device-status + HID keep running; no window events; GNOME fires a one-shot notify-send pointing to the extension.
```

### Known Gotchas of our codebase & Library Quirks

```text
// CRITICAL (GOTCHA-1 — do NOT hedge the coverage).
//   The specs (PLATFORMS.md §6–§10) and the PRD §5 matrix already state the broad
//   coverage as FACT (F16 is implemented — all 5 backends exist). The current docs'
//   "Hyprland only" / "not supported yet" wording is simply STALE, not a cautious
//   hedge. Replacing it with hedged language ("experimental", "best on Hyprland")
//   re-introduces the contradiction and suppresses adoption. State the coverage
//   plainly; cite qmkonnect -v as the way to confirm the selected backend.

// CRITICAL (GOTCHA-2 — the two GNOME problems are SEPARATE; do not conflate).
//   GNOME has TWO independent gaps, each with its own fix:
//     (a) WINDOW DETECTION: GNOME exposes no active-window API → needs the
//         user-installed 'qmkonnect@mulletware' Shell extension (PLATFORMS.md §8).
//         Symptom: no/wrong window events. Fix: install + enable the extension.
//     (b) TRAY ICON: stock GNOME dropped SNI/AppIndicator → the ksni tray item is
//         invisible. Symptom: no tray icon. Fix: install the 'AppIndicator and
//         KStatusNotifierItem Support' extension, OR run trayless (LINUX.md §7.4).
//   These are DIFFERENT extensions for DIFFERENT symptoms. Document them as two
//   distinct troubleshooting entries. Do NOT imply the window extension fixes the
//   tray, or vice versa. Do NOT suggest building a GNOME-native tray (spec says no).

// CRITICAL (GOTCHA-3 — AT-SPI is best-effort; name it as such).
//   AT-SPI (#4) is the LAST-ditch fallback, NOT a primary. Its app_class is the
//   app's readable Name (not WM_CLASS) → inconsistent for Electron/sandboxed apps;
//   titles vary; some apps are invisible. WITHOUT docs, a GNOME-without-extension
//   user lands on AT-SPI and sees wrong/empty names with no clue why. Document:
//   (a) AT-SPI is best-effort, (b) it needs accessibility ENABLED (most distros
//   ship a11y off — 'gsettings set org.gnome.desktop.interface
//   toolkit-accessibility true' or Settings → Accessibility), (c) install the
//   GNOME Shell extension for reliable detection. (PLATFORMS.md §9.)

// CRITICAL (GOTCHA-4 — PRESERVE the P2.M6.T1.S1 autostart subsection).
//   The parallel task P2.M6.T1.S1 adds '### Autostart at login' to docs/installation.md
//   (systemd service vs XDG .desktop, trade-off, Hidden=true disable, wlroots/dex).
//   This task MUST NOT rewrite, move, or duplicate it. Anchor your installation.md
//   edits on OTHER stable text (the '### Linux (Hyprland Only)' header, the intro
//   table, the Package Managers block). The F17 narrative in YOUR edits should be a
//   one-line cross-reference ("see Autostart at login below"), not a second telling.

// (GOTCHA-5 — the build is RED from parallel work; this task builds NOTHING).
//   `cargo build --release` currently fails (private `mod gnome;` reach in
//   runners/linux.rs, per P2.M6.T1.S1 GOTCHA-1 — owned by a sibling). This task is
//   100% markdown. Do NOT run cargo to "verify" it; do NOT touch any Rust file.

// (GOTCHA-6 — docs/llms_full.txt is OWNED by P2.M7.T2.S2).
//   The omnibus doc is regenerated by a later task. Editing docs/*.md does NOT
//   auto-update llms_full.txt — and you must NOT hand-edit it (it is a generated
//   artifact). Leave it; P2.M7.T2.S2 regenerates it from the source .md files.

// (GOTCHA-7 — docs/platforms/ does NOT exist; contract point (d) is N/A).
//   There is no docs/platforms/ directory and no docs/platforms.md. Do NOT create
//   one — the cross-DE detail lives in spec/PLATFORMS.md and is surfaced into
//   installation.md + troubleshooting.md by THIS task. Creating a new doc page
//   would be scope creep + would need wiring into the Jekyll nav (_config.yml,
//   index.md), which is out of scope. Surface the detail inline.

// (GOTCHA-8 — use the Jekyll-relative link syntax already in the docs).
//   docs/*.md uses `{{ site.baseurl }}/troubleshooting/` style internal links
//   (Jekyll permalinks), while README.md + spec/*.md use relative `docs/foo.md`.
//   Match the convention of the file you're editing — don't mix. New cross-links
//   inside docs/troubleshooting.md → installation.md use the baseurl form; links
//   from README.md use the relative form.

// (GOTCHA-9 — the macOS "Accessibility permissions" line in troubleshooting is a
//   PRE-EXISTING inaccuracy, NOT in scope here).
//   docs/troubleshooting.md 'Window Detection → macOS' says "Grant Accessibility
//   permissions (required for window monitoring)" — this is WRONG (macOS needs
//   Screen Recording, not Accessibility; corrected lower in the same file under
//   'macOS Issues'). This is a real bug but it is OUT OF SCOPE for the F16/F17
//   Linux docs task. Do NOT fix it here (avoid scope creep / unrelated diffs);
//   flag it for a separate macOS-docs task. (If you have spare budget and want to
//   be thorough, a one-line fix is defensible, but it is not required and must not
//   distract from the Linux deliverables.)
```

## Implementation Blueprint

### Data models and structure

None. Pure markdown edits.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: MODIFY docs/installation.md — the headline cross-DE overhaul
  - EDIT 1 (intro compat table, L27): replace
      | **Linux** (Hyprland) | binary / Arch PKGBUILD | AUR · Nix |
    with a row mirroring PRD §5:
      | **Linux** | GNOME · KDE Plasma · COSMIC · Hyprland · Sway · Niri · wlroots (Wayland); XFCE · MATE · Cinnamon · Budgie · LXQt (X11) | AUR · Nix · .deb/.rpm · PKGBUILD/binary | systemd user service + XDG autostart `.desktop` (F17) |
    (keep the table columns aligned with the Windows/macOS rows above it).
  - EDIT 2 (the "## Linux" section header + note, L99–L101): replace
      ### Linux (Hyprland Only)
      **Note**: QMKonnect currently only supports Hyprland on Linux. Other window managers are not supported yet. Please contribute support for your window manager!
    with:
      ### Linux
      QMKonnect supports **every major Linux desktop**. At startup it probes for a
      window-monitor backend and picks the right one automatically — you usually
      don't configure it. Run `qmkonnect -v` to see which backend was selected and
      why each other candidate was skipped.
      [INSERT the compact backend-selection table from §"The authoritative backend
       matrix" above — 5 rows, columns: # / Backend / Covers / Probe.]
      Override (rarely needed): set `[linux] backend = "…"` in `config.toml`
      (`foreign-toplevel | gnome | hyprland | atspi | x11 | auto`, default `auto`).
      See `spec/PLATFORMS.md` §6 for the full matrix.
      - **GNOME** needs the `qmkonnect@mulletware` Shell extension for window
        detection (Mutter exposes no active-window API) — see *GNOME (Shell
        extension)* below. The tray icon additionally needs the AppIndicator
        extension (see [Troubleshooting]({{ site.baseurl }}/troubleshooting/)).
      - **KDE Plasma 6, COSMIC, Hyprland, Sway, Niri** and the wlroots family
        work with **no extension** (foreign-toplevel protocol).
      - **XFCE, MATE, Cinnamon, Budgie, LXQt** work over **X11** (no extension).
  - EDIT 3 (preserve): DO NOT touch the P2.M6.T1.S1 `### Autostart at login`
    subsection (~L133). If your header rewrite lands above it, leave a natural
    flow; optionally add one cross-reference line ("Login autostart on systemd
    and non-systemd distros is covered in *Autostart at login* below.") — but do
    NOT re-explain F17 (GOTCHA-4).
  - EDIT 4 (the `### GNOME (optional Shell extension)` section, L191): KEEP its
    content (it is already correct) but drop "optional" framing now that GNOME is
    a first-class supported desktop — rename to `### GNOME (Shell extension)`.
    Ensure its intro references the backend-selection table above (it already
    notes "On every other desktop … no extension needed" — keep that).
  - EDIT 5 (`### Package Managers`, L289): after the existing AUR / Nix / mise
    blocks, ADD two blocks (commands from spec/PACKAGING.md §4.3/§4.4):
      **.deb (Debian / Ubuntu)** — built with `cargo-deb`:
        ```bash
        # from a release: download qmkonnect_<ver>_amd64.deb, then:
        sudo apt install ./qmkonnect_<ver>_amd64.deb
        # or build from source: cargo install cargo-deb && cargo deb
        # → target/debian/qmkonnect_*.deb
        ```
      **.rpm (Fedora / RHEL / openSUSE)** — built with `cargo-generate-rpm`:
        ```bash
        # from a release: download qmkonnect-<ver>.x86_64.rpm, then:
        sudo dnf install ./qmkonnect-<ver>.x86_64.rpm
        # or build from source: cargo install cargo-generate-rpm && cargo generate-rpm
        # → target/generate-rpm/*.rpm
        ```
  - VERIFY: `grep -in "hyprland only\|not supported yet" docs/installation.md` → empty.
  - VERIFY: `grep -in "select_linux_backend\|backend-selection\|qmkonnect -v\|\[linux\] backend" docs/installation.md` → ≥1 hit each.
  - VERIFY: `grep -in "Autostart at login" docs/installation.md` → still present (P2.M6.T1.S1 subsection intact).
  - GOTCHA-1/4/7: plain coverage; preserve autostart subsection; no new platforms page.

Task 2: MODIFY docs/troubleshooting.md — backend-selection diagnosis + new Linux entries
  - EDIT 1 (Window Detection → "#### Linux (Hyprland Only)", L203): replace the
    heading + body with a broad backend-selection diagnosis. New heading:
    `#### Linux`. Body:
      QMKonnect auto-selects a window backend at startup. If focus changes don't
      switch layers, first see **which backend it chose** and why:
      ```bash
      qmkonnect -v 2>&1 | grep -i "backend\|probing\|selected"
      # Expect: 'select_linux_backend: probing X…' lines + '… selected' for the winner.
      ```
      Then match your desktop:
      - **foreign-toplevel** (KDE Plasma 6, COSMIC, Hyprland, Sway, Niri, wlroots):
        no setup needed; if empty, your compositor may not advertise
        `wlr-foreign-toplevel` — confirm with `qmkonnect -v`.
      - **gnome** → needs the `qmkonnect@mulletware` Shell extension (see
        *Linux (GNOME)* below and [Installation → GNOME]({{ site.baseurl }}/installation/)).
      - **hyprland** (legacy IPC): verify the socket (commands retained below).
      - **atspi** (best-effort fallback): see *AT-SPI best-effort* under Linux
        Issues — names may be inconsistent; install the GNOME extension for
        reliable detection.
      - **x11** (XFCE/MATE/Cinnamon/Budgie/LXQt): needs `xprop`; verify with
        `which xprop`.
      [KEEP the existing Hyprland socket diagnostic commands as the Hyprland-IPC
       hint block — but DROP the "**Note**: Only Hyprland is supported…" line at L214.]
  - EDIT 2 (DELETE the stale note at L214): the "Only Hyprland is supported on
    Linux. Other window managers are not supported yet." line → removed.
  - EDIT 3 (KEEP "#### Linux (GNOME)", L216): it is already correct (extension
    diagnosis + `gdbus`/`gnome-extensions` checks). Optionally add a one-line
    pointer that wrong/empty app names → AT-SPI fallback (cross-link to the new
    AT-SPI entry).
  - EDIT 4 (DELETE the second stale note, L402): under "#### Hyprland Integration
    Issues", remove "Only Hyprland is supported on Linux. Other window managers
    are not supported yet. Please contribute support for your window manager!".
    Keep the socket-permission diagnostic above it.
  - EDIT 5 (ADD, under "### Linux Issues", L319): three new subsections:
      `#### No tray icon on GNOME`
        Stock GNOME dropped SNI/AppIndicator support, so QMKonnect's tray item is
        invisible on a default GNOME session (the daemon still runs fine). Two
        options:
        1. **Install the *AppIndicator and KStatusNotifierItem Support* extension**
           ([extensions.gnome.org](https://extensions.gnome.org/extension/615/appindicator-support/),
           or your distro's `gnome-shell-extension-appindicator` package). Enable
           it in the **Extensions** app, then **log out and back in** on Wayland.
        2. **Run trayless** — device status, rules, and settings all work from the
           CLI (`--list-devices`, `--validate-rules`, `--list-callbacks`); only the
           click-menu is unavailable.
        (This is independent of the window-detection Shell extension — see *Linux
        (GNOME)* under Window Detection for that.) `spec/LINUX.md` §7.4.
      `#### AT-SPI best-effort caveats / enable accessibility`
        The AT-SPI backend is a **last-ditch fallback** (mainly GNOME without the
        Shell extension). It is *best-effort*:
        - `app_class` is the app's readable **name**, not `WM_CLASS` — usually
          fine (`Firefox`) but inconsistent for Electron/sandboxed apps
          (`python3`, `chrome`, or empty).
        - Titles vary (the focused *accessible*, not the toplevel).
        - Apps that don't expose accessibility (some games, some Qt apps) are
          invisible.
        It also needs **accessibility enabled** (most distros ship it off). Turn it on:
        ```bash
        gsettings set org.gnome.desktop.interface toolkit-accessibility true
        # or: Settings → Accessibility → enable Screen Reader / Assistive Technology
        ```
        For reliable GNOME detection, **install the `qmkonnect@mulletware` Shell
        extension** instead (Installation → GNOME). `spec/PLATFORMS.md` §9.
      `#### GNOME shows wrong / inconsistent window names`
        If `qmkonnect -v` shows the **atspi** backend (not **gnome**) on GNOME,
        the Shell extension isn't installed/enabled and you're on the best-effort
        fallback → see *AT-SPI best-effort* above. Install + enable the
        `qmkonnect@mulletware` extension (Installation → GNOME) and restart
        QMKonnect; it will auto-select the **gnome** backend and report reliable
        `WM_CLASS`-based names.
  - VERIFY: `grep -in "hyprland only\|not supported yet\|please contribute support" docs/troubleshooting.md` → empty.
  - VERIFY: `grep -in "AppIndicator\|toolkit-accessibility\|best-effort\|atspi" docs/troubleshooting.md` → ≥1 hit each.
  - GOTCHA-2/3: keep the two GNOME problems separate; name AT-SPI best-effort.

Task 3: MODIFY README.md — Features + Installation + Package Managers table
  - EDIT 1 (Features, L21): replace
      - Linux: Arch/Hyprland only
    with:
      - Linux: GNOME, KDE Plasma, COSMIC, Hyprland, Sway, Niri, the wlroots family
        (Wayland), and XFCE/MATE/Cinnamon/Budgie/LXQt (X11). The backend is chosen
        automatically at runtime — run `qmkonnect -v` to see which. GNOME needs the
        `qmkonnect@mulletware` Shell extension for window detection.
  - EDIT 2 (Installation): broaden the "### Arch Linux" + "### Other Linux Systems"
    framing. Rename the block to `### Linux` and add a one-paragraph cross-DE intro
    (mirrors Task 1 EDIT 2, condensed) + a note that the backend is auto-selected
    (`qmkonnect -v`) and GNOME needs the extension (link to docs/installation.md#linux).
    Keep the Arch makepkg + binary/systemd/udev install steps (they're correct).
  - EDIT 3 (Package Managers table): ADD two rows after the Nix row:
      | **Debian / Ubuntu** | `.deb` | `sudo apt install ./qmkonnect_<ver>_amd64.deb` |
      | **Fedora / RHEL** | `.rpm` | `sudo dnf install ./qmkonnect-<ver>.x86_64.rpm` |
  - VERIFY: `grep -in "hyprland only" README.md` → empty.
  - VERIFY: `grep -in "GNOME, KDE Plasma, COSMIC" README.md` → ≥1 hit.
  - VERIFY: `grep -in "\.deb\|\.rpm" README.md` → the two new table rows.

Task 4 (OPTIONAL, adjacent): MODIFY docs/configuration.md — clarify "(Hyprland only)"
  - EDIT 1 (L74): `# (Hyprland only) periodic active-window poll interval (ms).`
    → `# (Hyprland IPC backend) periodic active-window poll interval (ms).`
  - EDIT 2 (L221): the table row note `(Hyprland only)` → `(Hyprland IPC backend)`.
  - WHY optional: `poll_interval_ms` IS only consumed by the Hyprland IPC backend
    (PLATFORMS.md §5.4), so the old note is technically correct — but it reads as
    "Linux = Hyprland only", contradicting F16. The clarifying edit removes the
    confusion at near-zero risk. If budget is tight, defer; it does not block the
    core deliverables.
  - VERIFY: `grep -in "Hyprland only" docs/configuration.md` → empty (after edit).
```

### Implementation Patterns & Key Details

```markdown
<!-- Cross-DE intro pattern (condensed backend table for installation.md / README) -->
QMKonnect auto-selects a Linux window backend at startup (first available wins).
Run `qmkonnect -v` to see the probe trace + the chosen backend.

| Backend | Covers | Needs |
|---|---|---|
| foreign-toplevel (Wayland) | KDE Plasma 6, COSMIC, Hyprland, Sway, Niri, wlroots | nothing |
| GNOME extension | GNOME | the `qmkonnect@mulletware` Shell extension (user-installed) |
| Hyprland IPC | Hyprland (legacy fallback) | nothing |
| AT-SPI | any compositor w/ a11y ON | accessibility enabled (best-effort) |
| X11 | XFCE, MATE, Cinnamon, Budgie, LXQt | `xprop` |

Override: `[linux] backend = "…"` in config.toml (default `auto`). Full detail:
`spec/PLATFORMS.md` §6.

<!-- The two GNOME fixes (always document separately — GOTCHA-2) -->
- **No window detection on GNOME** → install the `qmkonnect@mulletware` Shell
  extension (Installation → GNOME). [PLATFORMS.md §8]
- **No tray icon on GNOME** → install the *AppIndicator and KStatusNotifierItem
  Support* extension (https://extensions.gnome.org/extension/615/appindicator-support/),
  or run trayless (CLI flags). [LINUX.md §7.4]

<!-- Internal links: match the file's convention (GOTCHA-8) -->
- Inside docs/*.md  → `{{ site.baseurl }}/troubleshooting/`  (Jekyll permalink)
- Inside README.md  → `docs/installation.md#linux`           (relative path)
```

### Integration Points

```yaml
DOCS CROSS-LINKS (ensure these resolve):
  - docs/troubleshooting.md → docs/installation.md (GNOME section): use {{ site.baseurl }}/installation/#linux
  - docs/installation.md → docs/troubleshooting.md (GNOME tray): use {{ site.baseurl }}/troubleshooting/
  - README.md → docs/installation.md#linux : relative path docs/installation.md#linux

SPEC CROSS-REFS (cite in prose, do NOT edit specs):
  - spec/PLATFORMS.md §6 (backend selection), §8 (GNOME extension), §9 (AT-SPI), §10 (X11)
  - spec/LINUX.md §7.4 (AppIndicator tray), §6.3 (F17 autostart — owned by P2.M6.T1.S1)

NO STRUCTURAL CHANGES:
  - Do NOT create docs/platforms.md (GOTCHA-7).
  - Do NOT add new pages to the Jekyll nav (_config.yml / docs/index.md) — out of scope.
  - Do NOT regenerate docs/llms_full.txt (GOTCHA-6 — owned by P2.M7.T2.S2).
```

## Validation Loop

> Docs-only task. **Do NOT run `cargo build`** (the default-features build is RED
> from parallel work — GOTCHA-5 — and is irrelevant). The gates are grep + link
> checks + markdown structure sanity.

### Level 1: Stale-string purge (the headline gate)

```bash
# The three core files must contain ZERO stale "Hyprland only" claims:
grep -rin "hyprland only\|only supports hyprland\|other window managers are not supported\|please contribute support for your window manager" \
  README.md docs/installation.md docs/troubleshooting.md
# Expected: NO output (exit 1). Any hit = a stale claim survived → fix it.

# (Optional, only if Task 4 was done) configuration.md:
grep -in "hyprland only" docs/configuration.md
# Expected: NO output.
```

### Level 2: New content present (grep gates)

```bash
# installation.md — cross-DE intro + backend table + qmkonnect -v + [linux] backend + .deb/.rpm:
grep -in "GNOME · KDE Plasma · COSMIC\|select_linux_backend\|qmkonnect -v\|\[linux\] backend\|cargo-deb\|cargo-generate-rpm" docs/installation.md
# Expected: ≥1 hit for each.

# troubleshooting.md — the three new Linux entries:
grep -in "AppIndicator\|toolkit-accessibility\|best-effort\|KStatusNotifierItem" docs/troubleshooting.md
# Expected: ≥1 hit for each (AppIndicator, toolkit-accessibility, best-effort).

# README.md — Features broadened + Package Managers .deb/.rpm:
grep -in "GNOME, KDE Plasma, COSMIC\|\.deb\|\.rpm" README.md
# Expected: the broadened Features line + the two new table rows.

# P2.M6.T1.S1 autostart subsection PRESERVED (critical scope guard):
grep -in "Autostart at login\|XDG autostart\|Hidden=true" docs/installation.md
# Expected: still present (the parallel task's subsection is intact).
```

### Level 3: Markdown structure + link integrity

```bash
# Heading hierarchy sanity (no broken nesting from the rewrites):
grep -n "^#" docs/installation.md docs/troubleshooting.md README.md | grep -i "linux"
# Expected: clean ##/###/#### nesting; no duplicate or orphaned Linux headings.

# Internal links resolve (Jekyll baseurl form in docs/, relative in README):
grep -ohE "\{\{ site\.baseurl \}\}/[a-z]+/" docs/installation.md docs/troubleshooting.md | sort -u
# Expected: only /installation/ and /troubleshooting/ (both are real permalinks —
# confirm in docs/*.md front matter `permalink:`).

# External URLs are the verified ones (Level 4 spot-check optional):
grep -oE "https://extensions.gnome.org/extension/615/appindicator-support/" docs/troubleshooting.md docs/installation.md
# Expected: the exact AppIndicator URL (copy-paste, don't paraphrase).
```

### Level 4: Scope-boundary guard (catch stray edits to sibling/spec-owned files)

```bash
git diff --stat
# Expected: ONLY markdown files. Concretely (core): README.md, docs/installation.md,
# docs/troubleshooting.md; (optional): docs/configuration.md. NOTHING else.

# Negative — NONE of these changed:
git diff --stat -- \
  spec docs/llms_full.txt PRD.md tasks.json plan/007_fb356ba503b4/prd_snapshot.md .gitignore \
  src packaging .github
# Expected: EMPTY.

# Confirm no Rust / packaging / CI touched:
git diff --name-only | grep -vE '\.md$'
# Expected: EMPTY (every changed file ends in .md).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 1 stale-string purge passes (zero "Hyprland only" hits in the 3 core files).
- [ ] All Level 2 grep gates pass (cross-DE intro, backend table, `qmkonnect -v`,
      `[linux] backend`, `.deb`/`.rpm`, AppIndicator, toolkit-accessibility, best-effort).
- [ ] P2.M6.T1.S1 `### Autostart at login` subsection is intact (grep confirms).
- [ ] `git diff --stat` → only `.md` files; Level 4 negative-diff is EMPTY.

### Feature (contract) Validation
- [ ] `docs/installation.md` `## Linux` lists all supported desktops + the 5-row
      backend-selection table + `qmkonnect -v` + `[linux] backend` override.
- [ ] `docs/troubleshooting.md` has the three new Linux entries: "No tray icon on
      GNOME" (AppIndicator), "AT-SPI best-effort / enable accessibility", "GNOME
      shows wrong/inconsistent window names".
- [ ] The two GNOME problems (window extension vs tray AppIndicator) are documented
      as **separate** entries (GOTCHA-2).
- [ ] `README.md` Features line + Package Managers table reflect broad coverage +
      `.deb`/`.rpm`.
- [ ] AT-SPI is named **best-effort** with the a11y-enable requirement + the
      `gsettings` command (GOTCHA-3).

### Scope & Code-Quality Validation
- [ ] NO edits to `spec/*.md` (human-owned, read-only reference).
- [ ] NO edit to `docs/llms_full.txt` (GOTCHA-6 — owned by P2.M7.T2.S2).
- [ ] NO edit to the P2.M6.T1.S1 autostart subsection (GOTCHA-4 — preserved).
- [ ] NO new `docs/platforms.md` page created (GOTCHA-7).
- [ ] NO Rust / packaging / CI edits; NO `cargo build` run (GOTCHA-5).
- [ ] Internal links match each file's convention (Jekyll baseurl in docs/, relative
      in README — GOTCHA-8).

### Documentation & Deployment
- [ ] Coverage is stated **plainly** (no hedging — GOTCHA-1); a GNOME/KDE/Sway/XFCE
      user reads "my desktop is supported" without caveats.
- [ ] Every GNOME user-facing claim cross-links to the install/troubleshoot fix.
- [ ] Wording is consistent across README ↔ installation ↔ troubleshooting (same
      backend names, same GNOME-extension/AppIndicator names).

---

## Anti-Patterns to Avoid

- ❌ Don't **hedge** the coverage ("experimental", "best on Hyprland", "primarily
  Hyprland") — the specs + PRD state it as fact and F16 is implemented. Hedging
  re-introduces the contradiction the task exists to fix (GOTCHA-1).
- ❌ Don't **conflate** the two GNOME problems. Window detection (Shell extension)
  and tray icon (AppIndicator extension) are different symptoms with different
  fixes — document them as distinct entries (GOTCHA-2).
- ❌ Don't present **AT-SPI as a primary** backend or omit its caveats — it is
  best-effort and needs accessibility enabled (GOTCHA-3). Name it as such.
- ❌ Don't **rewrite/move/duplicate** the P2.M6.T1.S1 `### Autostart at login`
  subsection — it is owned by the parallel task; preserve it and at most cross-
  reference it (GOTCHA-4).
- ❌ Don't run `cargo build` or touch any Rust file — this is 100% markdown and the
  build is RED from parallel work anyway (GOTCHA-5).
- ❌ Don't hand-edit `docs/llms_full.txt` — it's a generated artifact owned by
  P2.M7.T2.S2 (GOTCHA-6).
- ❌ Don't create `docs/platforms.md` or touch the Jekyll nav — contract point (d)
  is N/A; the detail goes inline in installation/troubleshooting (GOTCHA-7).
- ❌ Don't fix the unrelated macOS "Accessibility permissions" inaccuracy in
  troubleshooting.md — it's out of scope for the F16/F17 Linux task (GOTCHA-9);
  flag it separately. (A one-line fix is defensible if budget allows, but it must
  not dilute the Linux deliverables.)
- ❌ Don't mix link conventions — Jekyll `{{ site.baseurl }}/x/` inside `docs/*.md`,
  relative `docs/x.md` inside `README.md` (GOTCHA-8).
- ❌ Don't paraphrase the AppIndicator extension name or URL — copy "AppIndicator
  and KStatusNotifierItem Support" + the EGO link verbatim.

---

**Confidence Score: 9/10** for one-pass completion. The deliverable is pure
markdown where every edit site is pinned to an exact stale string + line number
(grep-verified), every replacement is sourced verbatim from the authoritative
`spec/` + PRD §5 matrix (reproduced in the PRP), and the external citations
(AppIndicator EGO page, AT-SPI `gsettings` command) are confirmed. The validation
loop is non-Rust grep + link checks, so the parallel `cargo build` redness cannot
interfere. The two residual risks are (1) colliding with the P2.M6.T1.S1 autostart
subsection in `docs/installation.md` — guarded explicitly by GOTCHA-4 + a Level 2
grep gate that the subsection survives; and (2) an agent hedging the coverage or
conflating the two GNOME problems — guarded by GOTCHA-1/2 + §Anti-Patterns. Both
are addressed with ready-to-paste prose so no judgment is required at edit time.