# Research Notes — P1.M6.T2.S2

## Regenerate docs/llms_full.txt and update PACKAGING spec references

### 1. The regeneration script (`docs/generate_llms_full.sh`)

Concatenates exactly 8 files, in order, into `docs/llms_full.txt`, each wrapped
in an 80-char `=` divider with an `emit "<n>. <file>" (<label>)` header:

1. `README.md` (repo root — no Jekyll front matter)
2. `docs/index.md` (Home)
3. `docs/installation.md` (Installation) ← updated by P1.M6.T1.S1
4. `docs/qmk-integration.md`
5. `docs/configuration.md`
6. `docs/usage.md`
7. `docs/examples.md`
8. `docs/troubleshooting.md`

- `strip_fm()` strips a **leading** Jekyll `--- ... ---` front-matter block only.
- `set -euo pipefail`. Prints `wrote .../llms_full.txt (<lines> lines, <bytes> bytes)`.
- **Does NOT** include `spec/*.md` or `packaging/**` — intentional (user-docs bundle).
- **Output is git-tracked** (`git ls-files docs/llms_full.txt` returns it; not in
  `docs/.gitignore`, which only ignores `_site/ .sass-cache .jekyll-cache
  .jekyll-metadata .bundle/ vendor/`). ⇒ regeneration yields a committable diff.

### 2. Staleness confirmed (the core problem)

| artifact | mtime | community-channel mentions |
|---|---|---|
| `docs/llms_full.txt` | Aug 5 01:09 | 1 (STALE) |
| `README.md` | Aug 7 02:45 | 10 (has "Package Managers" §, L107–128) |
| `docs/installation.md` | Aug 7 02:34 | updated (commit ff58ac8) |

`README.md` "Package Managers" table (L115–120): AUR `qmkonnect-bin`, Nix flake,
Homebrew Cask, Scoop, Winget, mise/asdf. None of this is in the current bundle.
After regen, the community-channel grep count must rise from 1 → ≥8.

### 3. spec/PACKAGING.md ↔ packaging/ structure — verified mapping

**COMPLETED channels (all paths exist):**

| spec claim | path | exists? |
|---|---|---|
| §4.1 Arch source PKGBUILD | `packaging/linux/arch/PKGBUILD` | ✅ |
| §4.2 AUR (`qmkonnect-bin`) | `packaging/linux/aur/` (PKGBUILD+.SRCINFO+publish.sh) | ✅ |
| §4.5 Nix flake (**repo root**) | `flake.nix` (+ `packaging/nix/README.md`) | ✅ |
| §6.1 Homebrew Cask | `packaging/homebrew/Casks/qmkonnect.rb` | ✅ |
| §6.2 Scoop | `packaging/scoop/qmkonnect.json` | ✅ |
| §6.3 Winget | `packaging/winget/dabstractor.QMKonnect{,.installer,.locale.en-US}.yaml` | ✅ |
| §6.4 mise + asdf | `packaging/asdf/` (bin/{install,download,list-all} + mise.toml) | ✅ |

**FUTURE-STATE sections (DO NOT flag as missing — owned by other subtasks):**

| spec claim | path | status |
|---|---|---|
| §4.3 `.deb` via cargo-deb — `— NEW` | `packaging/debian/` | absent → P1.M7.T1 |
| §4.4 `.rpm` via cargo-generate-rpm — `— NEW` | `packaging/rpm/` | absent → P1.M7.T2 |
| §4.7 XDG `.desktop` — `— NEW` | `packaging/linux/xdg/` | absent → P2.M6.T1 |

The spec describes target end-state; these `— NEW` markers are intentional.

### 4. Packaging internal docs that cross-link to the spec (4 files)

```
packaging/linux/aur/README.md:64    → spec/PACKAGING.md §4   (CORRECT — AUR is §4.2)
packaging/scoop/README.md:~140      → spec/PACKAGING.md §3   (defensible — §3 = Inno/WP, which Scoop wraps; §6.2 is the channel itself)
packaging/scoop/bucket-README.md:~146 → spec/PACKAGING.md §3 (absolute GitHub URL; same as above)
packaging/winget/README.md:~160     → spec/PACKAGING.md §3   (defensible — same as Scoop)
```

**STALE FORWARD-REFERENCE FOUND** — `packaging/winget/README.md`, "Install docs"
cross-link (~line 154):
```
- **Install docs:** [`docs/installation.md`](../../docs/installation.md)
  (Windows section — a Winget row is added in P1.M6.T1.S1)
```
P1.M6.T1.S1 is now **Complete**; `docs/installation.md` already has the Winget
row. → Fix: rewrite the parenthetical to present tense (or drop it).

### 5. Validation commands (confirmed working shape)

```bash
cd docs && ./generate_llms_full.sh && cd ..
test docs/llms_full.txt -nt README.md                                  # freshness
n=$(grep -Eic 'homebrew|scoop|winget|brew install|nix run|asdf|aur' docs/llms_full.txt); [ "$n" -ge 8 ]  # content (was 1)
git diff --stat docs/llms_full.txt                                     # non-empty diff
cargo test --bin qmkonnect -- --test-threads=1                         # regression guard
```
No `.rs` is touched in this task, so the test run is a pure regression check.
Falls back to `cargo check --bin qmkonnect` only if the host lacks Linux build
deps (document the reason).

### 6. Scope guard

This is a **docs-sync tail** task. Do NOT:
- edit `spec/PACKAGING.md` (source of truth, read-only here);
- edit `docs/generate_llms_full.sh` (already correct);
- add spec/packaging content into `llms_full.txt` (wrong design);
- report `.deb`/`.rpm`/`.desktop` absence as defects (P1.M7/P2.M6).