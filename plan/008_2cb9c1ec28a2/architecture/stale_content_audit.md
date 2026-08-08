# Stale Content Audit — mise/asdf References

## Audit Methodology

Each claim in the Delta PRD was independently verified against the current
codebase using `grep`, `read`, and `ls` commands. Four parallel scout subagents
cross-validated all findings.

---

## 1. docs/installation.md — STALE (3 blocks, 12 grep hits)

### Block 1: Intro paragraph (DELETE lines 29–32)

```
29: **mise / asdf** are cross-platform version managers that install the prebuilt release binary:
30: **Linux** (full app) and **macOS** (**CLI only — no menu-bar tray**); not available on Windows.
31: See the per-platform sections.
32: (blank)
```

- Preceded by: platform overview table (lines 23–27) + blank line (28).
- Followed by: `## Windows` heading (line 33).
- After deletion: blank line (28) → `## Windows` (33). Correct spacing.

### Block 2: Linux "mise / asdf" subsection (DELETE lines 289–301)

```
289: **mise / asdf** — cross-platform version managers. The same `asdf-qmkonnect` plugin serves both
290: (mise runs asdf plugin scripts unchanged). **Linux is fully supported** — install the binary, then
291: run the one-time udev/systemd setup the plugin documents:
292: (blank)
293: ```bash
294: # asdf:
295: asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
296: asdf install qmkonnect latest
297: # mise:
298: mise plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
299: mise install qmkonnect@latest
300: ```
301: (blank)
```

- Preceded by: Nix flake README paragraph (lines 286–287) + blank (288).
- Followed by: `**.deb (Debian / Ubuntu)**` section (line 302).
- After deletion: blank line (288) → .deb section (302). Correct spacing.
- Contains 4 broken links to `https://github.com/dabstractor/asdf-qmkonnect`.

### Block 3: macOS "mise / asdf — CLI only" subsection (DELETE lines 367–376)

```
367: **mise / asdf — CLI only (no menu-bar tray).** These install the raw Mach-O binary from the DMG,
368: which runs CLI flags (`--help`, `--list-callbacks`, `-r`, …) but **not** the menu-bar tray/icon —
369: that needs the full `.app` bundle. For the complete macOS app, use the **Homebrew cask** above or
370: the **direct DMG** instead:
371: (blank)
372: ```bash
373: asdf plugin add qmkonnect https://github.com/dabstractor/asdf-qmkonnect
374: asdf install qmkonnect latest        # CLI only — no menu-bar app
375: ```
376: (blank)
```

- Preceded by: Homebrew uninstall paragraph (lines 364–365) + blank (366).
- Followed by: `### Launch at login` heading (line 377).
- After deletion: blank line (366) → `### Launch at login` (377). Correct spacing.
- Contains 1 broken link to `https://github.com/dabstractor/asdf-qmkonnect`.

### Verification command (post-edit):

```bash
grep -in 'mise\|asdf' docs/installation.md      # MUST return zero hits
grep -in 'asdf-qmkonnect' docs/installation.md   # MUST return zero hits
```

---

## 2. docs/llms_full.txt — STALE (14 grep hits)

| Line(s) | Source | Content | Fix |
|---|---|---|---|
| 157 | Old README.md snapshot | Table row: `\| **Linux / macOS** \| mise · asdf \| …` | Regenerate (README already clean) |
| 160 | Old README.md snapshot | Caveat: `> - **mise / asdf on macOS is CLI-only**…` | Regenerate (README already clean) |
| 490 | installation.md §29 | `**mise / asdf** are cross-platform version managers…` | Fix installation.md → regenerate |
| 750–760 | installation.md §289–301 | Linux mise/asdf subsection + code block | Fix installation.md → regenerate |
| 828–835 | installation.md §367–376 | macOS mise/asdf subsection + code block | Fix installation.md → regenerate |

**Critical ordering:** Installation.md must be edited BEFORE regenerating.
Regenerating alone would only fix lines 157/160 (the README staleness) — the
installation.md-derived lines (490, 750–760, 828–835) would persist because the
source file is still stale.

---

## 3. All Other Files — ALREADY SYNCED (verified)

| File | Status | Verification |
|---|---|---|
| `spec/PACKAGING.md` | ✅ SYNCED | §6.4 (L393) = "mise / asdf — NOT a channel". No stale refs outside §6.4. |
| `spec/PRD.md` | ✅ SYNCED | F15 (L152), §2.1 (L97), §5 (L169) all exclude mise/asdf as a channel. |
| `README.md` | ✅ SYNCED | Zero mise/asdf grep hits. |
| `.github/workflows/release.yml` | ✅ SYNCED | Zero asdf/mise grep hits. No CI job. |
| `packaging/asdf/` | ✅ REMOVED | Directory does not exist. |
| `src/platforms/linux.rs` | ✅ IMPLEMENTED | `ensure_xdg_autostart` at L619. |
| `src/runners/linux.rs` | ✅ IMPLEMENTED | Called at L39. |
| `src/*.rs` + `*.toml` + `*.yml` + `*.sh` + `*.json` | ✅ CLEAN | Zero standalone mise/asdf refs. |
| Other `docs/*.md` (index, qmk-integration, configuration, usage, examples, troubleshooting) | ✅ CLEAN | Zero mise/asdf refs. |

---

## 4. docs/generate_llms_full.sh — Generator Analysis

**Executable:** Yes (`0755`, owner-executable).
**Invocation:** `bash docs/generate_llms_full.sh`
**Output:** `docs/llms_full.txt`

**Concatenation order (hardcoded — NOT a glob):**

1. `README.md` (repo root)
2. `docs/index.md`
3. `docs/installation.md`
4. `docs/qmk-integration.md`
5. `docs/configuration.md`
6. `docs/usage.md`
7. `docs/examples.md`
8. `docs/troubleshooting.md`

**Front-matter handling:** `strip_fm()` strips a leading Jekyll `--- … ---` block.
**Exclusions:** `docs/vendor/` is never read (explicit list, no globbing).
**Post-run verification:** Script prints `wrote docs/llms_full.txt (<N> lines, <M> bytes)`.