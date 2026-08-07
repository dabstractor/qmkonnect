# Research Notes — P2.M7.T2.S2 (Regenerate llms_full.txt + final build verification)

## 1. What `docs/generate_llms_full.sh` ACTUALLY does

**CRITICAL CORRECTION to the work-item contract:** the contract says the script
"concatenates spec docs + README + source into docs/llms_full.txt." **This is
inaccurate.** The script concatenates **ONLY**:

1. `README.md` (repo root — no Jekyll front matter)
2. `docs/index.md`
3. `docs/installation.md`
4. `docs/qmk-integration.md`
5. `docs/configuration.md`
6. `docs/usage.md`
7. `docs/examples.md`
8. `docs/troubleshooting.md`

It does **NOT** pull in `spec/*.md`, `src/**/*.rs`, or any `packaging/**`.
`strip_fm()` strips a LEADING Jekyll `--- ... ---` front-matter block if line 1
is `---` (the 7 `docs/*.md` have front matter; README does not → passed through).

Properties: `set -euo pipefail`; uses `BASH_SOURCE` to locate its own dir → **runs
from anywhere** (`bash docs/generate_llms_full.sh` from root, OR `cd docs &&
./generate_llms_full.sh`). **Idempotent + deterministic** for fixed inputs → no
spurious diff churn. Pure bash/awk/cat → **OS-independent** (same output on
Linux/macOS). Mode `0755` (executable).

## 2. `docs/llms_full.txt` IS git-tracked

`git ls-files docs/llms_full.txt` → tracked (NOT gitignored; `docs/.gitignore`
only covers Jekyll `_site/`, `.bundle/`, `vendor/`). Current size: 3014 lines,
~119 KB. **Regeneration therefore produces a real `git diff docs/llms_full.txt`
that is part of the deliverable** (committed with the P2 milestone).

## 3. Dependency on P2.M7.T1.S1 (sibling doc task)

P2.M7.T1.S1 (status: Ready) MODIFIES exactly the files that feed llms_full.txt:
- `README.md` ✓ (input #1)
- `docs/installation.md` ✓ (input #3)
- `docs/troubleshooting.md` ✓ (input #8)
- `docs/configuration.md` (optional, two annotations) ✓ (input #5)

So this task MUST run AFTER P2.M7.T1.S1's markdown edits are in place, else the
regen captures stale text. If the agent finds P2.M7.T1.S1 not yet landed, it
regenerates anyway (idempotent + harmless) but should note the dependency.

## 4. Build verification commands (verified conventions)

- **Tests:** `cargo test --bin qmkonnect -- --test-threads=1`
  - `--test-threads=1` is **MANDATORY** (AGENTS.md both macOS + Windows loops):
    the notifier/debouncer shares global mutable state → concurrent tests race.
  - 463 `#[test]` functions across `src/core/{types,pattern,rules,notifier,mod}.rs`
    + `src/platforms/{windows,hyprland,linux,x11,mod}.rs`. The `--bin qmkonnect`
    scope runs the crate's inline `#[cfg(test)]` modules.
- **Clippy:** `cargo clippy --all-targets -- -D warnings`
  - No `[lints]` table, no `.cargo/config.toml` clippy config, no `#![deny]` in
    lib → the explicit `-D warnings` IS the gate. `--all-targets` covers the test
    modules too.
  - **Host-OS caveat:** the F16 backends (`wayland_ft.rs`, `gnome.rs`,
    `atspi.rs`) are `#[cfg(target_os="linux")]`-gated → only exercised by clippy
    ON LINUX. On macOS/Windows the Linux backend code is cfg'd out. Ideally run
    on Linux; the per-OS CI (`ci.yml`) covers the others.

## 5. PRD §5 compatibility matrix → backend traceability (THE verification)

PRD §5 Linux row claims: GNOME / KDE Plasma / COSMIC / Hyprland / Sway / Niri /
wlroots (Wayland); XFCE / MATE / Cinnamon / Budgie / LXQt (X11).

`src/platforms/linux.rs::linux_backend_candidates()` priority order (all
default-on; X11 unconditional, always last):

| # | Candidate name     | Cargo feature | construct_backend arm     |
|---|--------------------|---------------|---------------------------|
| 1 | foreign-toplevel   | wayland       | wayland_ft::WaylandFtMonitor |
| 2 | gnome              | gnome         | gnome::GnomeMonitor       |
| 3 | hyprland           | hyprland      | hyprland::HyprlandMonitor |
| 4 | atspi              | atspi         | atspi::AtspiMonitor       |
| 5 | x11                | (uncond.)     | x11::X11Monitor           |

`default = ["wayland","gnome","atspi","hyprland","macos","linux-tray"]` → on
Linux all five are compiled in. DE → backend coverage:

| PRD §5 DE         | Backed by                          |
|-------------------|------------------------------------|
| GNOME             | gnome (#2)                         |
| KDE Plasma        | foreign-toplevel wayland (#1)      |
| COSMIC            | foreign-toplevel wayland (#1)      |
| Hyprland          | foreign-toplevel (#1) + hyprland IPC (#3 legacy) |
| Sway              | foreign-toplevel wayland (#1)      |
| Niri              | foreign-toplevel wayland (#1)      |
| wlroots family    | foreign-toplevel wayland (#1)      |
| X11 tail (XFCE/MATE/Cinnamon/Budgie/LXQt) | x11 (#5, unconditional) |

**All eight §5 claims are backed.** This is a confirm-and-document step — there
is NO automated test; the agent re-confirms the mapping via grep and records it.
PRD.md is READ-ONLY: if a claim were ever unbacked the agent could only FLAG it.

## 6. Out of scope / boundary

- P2.M7.T2.S1 (parallel, Implementing) edits ONLY `.github/workflows/release.yml`
  → does NOT touch any llms_full.txt input file → does NOT affect this regen.
- spec/*.md are human-owned, read-only, and NOT inputs to llms_full.txt anyway.
- If clippy fails on the new F16 backend source, FIXING the lint (behavior-
  preserving) is in-scope for this final milestone gate.