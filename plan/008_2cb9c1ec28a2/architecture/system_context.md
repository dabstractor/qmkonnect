# System Context — Delta #7: Remove mise/asdf as a Distribution Channel + Sync Docs

## Delta Summary

**Scope:** Documentation-only. No source-code or packaging change required.
**Trigger:** PRD v0.2.8 snapshot #6 → #7 decision: mise/asdf are removed from
F15 (the community package-manager distribution feature). They are now explicitly
documented as "NOT a channel (category mismatch)" in spec/PACKAGING.md §6.4.
A secondary change — `ensure_xdg_autostart()` for binary-only Linux installs — is
**already implemented in code** and requires no action.

## What Is Already Done (Verified)

| Item | State | Evidence |
|---|---|---|
| `ensure_xdg_autostart()` | ✅ implemented | `src/platforms/linux.rs:619` — `pub fn ensure_xdg_autostart(verbose: bool)` |
| Called from runner | ✅ implemented | `src/runners/linux.rs:39` — `crate::platforms::ensure_xdg_autostart(self.verbose);` |
| `packaging/asdf/` directory | ✅ removed | `ls packaging/asdf/` → "No such file or directory" |
| asdf/mise CI job | ✅ removed | `grep -in 'asdf\|mise' .github/workflows/release.yml` → no output (exit 1) |
| `spec/PRD.md` | ✅ synced | F15 row (L152) excludes mise/asdf; §2.1 (L97), §5 (L169) consistent |
| `spec/PACKAGING.md` | ✅ synced | §6.4 (L393–406) = "mise / asdf — NOT a channel (category mismatch)" |
| `README.md` | ✅ synced | `grep -in 'mise\|asdf' README.md` → no output |
| No other source/config refs | ✅ clean | Zero standalone mise/asdf refs in `.rs`/`.toml`/`.yml`/`.sh`/`.json` files |

## What Still Needs Doing

The **only stale content** is in the user-facing Jekyll docs. Two files:

### 1. `docs/installation.md` — Three mise/asdf blocks to delete

| Block | Lines | Content | Surrounding Context (preserved) |
|---|---|---|---|
| Intro paragraph | 29–32 | `**mise / asdf** are cross-platform version managers…` + platform note + `See the per-platform sections.` + trailing blank | Preceded by platform table (L23–27). Followed by `## Windows` (L33). |
| Linux subsection | 289–301 | `**mise / asdf** — cross-platform version managers…` + ```` ```bash ```` code block with `asdf plugin add` / `mise install` commands + trailing blank | Preceded by Nix flake README paragraph (L287). Followed by `**.deb (Debian / Ubuntu)**` section (L302). |
| macOS subsection | 367–376 | `**mise / asdf — CLI only (no menu-bar tray).**` + explanation + ```` ```bash ```` code block + trailing blank | Preceded by Homebrew uninstall paragraph (L364–366). Followed by `### Launch at login` (L377). |

**12 grep hits** (`grep -in 'mise\|asdf'`) — all within these 3 blocks.
**4 broken links** to `https://github.com/dabstractor/asdf-qmkonnect` — all within blocks 2 and 3.

**MUST NOT touch:** Nix (L278–287), .deb (L302–313), .rpm (L315+), Homebrew (L355–366),
Scoop, Winget, AUR sections. These are correct.

### 2. `docs/llms_full.txt` — Stale generated artifact

**14 grep hits** at lines 157, 160, 490, 750–760, 828–835.
- Lines 157, 160: from an **old README.md snapshot** (the live README is already clean — regenerating fixes this).
- Lines 490, 750–760, 828–835: mirror the stale `docs/installation.md` content (fixing installation.md first, then regenerating, fixes this).

**Regeneration:** `bash docs/generate_llms_full.sh` (executable, `0755`). The script
concatenates 8 files in a hardcoded order (README.md + 7 docs/*.md), strips Jekyll
front-matter, and writes to `docs/llms_full.txt`. It does NOT glob — `docs/vendor/`
is excluded by design.

## Architecture Decision: Documentation Mode

Per §5 of the SOW:
- **Mode A (doc-with-work):** S1's edit to `docs/installation.md` IS the doc work. There is no code change to document.
- **Mode B (changeset-level docs):** S2's regeneration of `docs/llms_full.txt` IS the changeset-level sync. It must run AFTER S1.
- A final T2 task verifies the README.md and all overview docs remain correctly synced.