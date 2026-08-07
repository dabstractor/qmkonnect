# PRP — P1.M6.T2.S2: Regenerate docs/llms_full.txt and update PACKAGING spec references

---

## Goal

**Feature Goal**: Refresh the agent-facing single-file documentation bundle
(`docs/llms_full.txt`) so it reflects the community package-manager channels
(AUR / Nix / Homebrew / Scoop / Winget / mise+asdf) added to `README.md` and
`docs/installation.md` in P1.M6.T1.S1 + P1.M6.T2.S1, AND verify that the
`spec/PACKAGING.md` F15 coverage stays consistent with the actual
`packaging/` directory structure — fixing any stale references in the
packaging internal docs that point back at the spec.

**Deliverable**:
1. Regenerated `docs/llms_full.txt` (a git-tracked, committed artifact) that
   contains the new "Package Managers" content.
2. A documented verification that every **completed** F15 channel claimed in
   `spec/PACKAGING.md` §6 (and AUR/Nix in §4.2/§4.5) maps to a real
   `packaging/` path.
3. Stale forward-references in `packaging/**/README.md` corrected.
4. `cargo test --bin qmkonnect -- --test-threads=1` passing (pure regression
   guard — no Rust source is touched).

**Success Definition**: `docs/llms_full.txt` mtime > `README.md` mtime, the new
community-channel strings appear in `llms_full.txt`, every completed-channel spec
claim resolves to an existing file, the winget README no longer references
P1.M6.T1.S1 as future work, and the single-threaded test suite is green.

## Why

- **Context-is-king for agents**: `docs/llms_full.txt` is the canonical
  single-file reference fed to LLMs/agents about QMKonnect. A stale bundle
  (built before the community-channel docs landed) means any agent reading it
  believes "no AUR package / no package managers" — directly contradicting the
  shipped F15 channels and the README's "Package Managers" table.
- **Spec ↔ reality drift**: `spec/PACKAGING.md` is the source of truth the
  packaging internal docs cross-link to (`packaging/{scoop,winget,linux/aur}/`
  READMEs each cite a `spec/PACKAGING.md` section). As the packaging tree grew,
  some cross-links and one forward-reference went stale; this task re-syncs them.
- **Scope boundary**: This is the **documentation-sync tail** of the F15 epic
  (P1.M6). It does NOT implement `.deb`/`.rpm`/`.desktop` (those are P1.M7 /
  P2.M6, still described in the spec as future state marked `— NEW`).

## What

User-/agent-visible behavior:
- `docs/llms_full.txt` gains the README "Package Managers" section + the
  installation-guide community-channel subsections.
- `packaging/winget/README.md` stops saying a Winget install-docs row "is added
  in P1.M6.T1.S1" (that subtask is **Complete**).
- No runtime/behavioral change whatsoever — documentation and a regenerated
  concatenation only.

### Success Criteria

- [ ] `cd docs && ./generate_llms_full.sh` exits 0 and reports new line/byte counts.
- [ ] `grep -c` of community-channel terms in `docs/llms_full.txt` rises from 1
      to ≥8 (matching README.md's count).
- [ ] `git diff --stat docs/llms_full.txt` shows a non-empty, content-bearing diff.
- [ ] Every **completed** F15 channel in `spec/PACKAGING.md` §6 + §4.2 + §4.5
      maps to an existing `packaging/` (or repo-root `flake.nix`) path — recorded
      in the verification table below.
- [ ] The spec's `— NEW` future-state sections (§4.3 `.deb`, §4.4 `.rpm`, §4.7
      XDG `.desktop`) are **left untouched** (P1.M7 / P2.M6 own them).
- [ ] `packaging/winget/README.md` no longer references P1.M6.T1.S1 as pending.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` passes (no regressions).

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything
> needed to implement this successfully?" — **YES.** The regeneration is one
> script invocation; the verification is a fixed mapping table; the doc fix is a
> single line edit. All file paths, section numbers, and decision rules are
> enumerated below.

### Documentation & References

```yaml
# MUST READ — the regeneration script (the heart of this task)
- file: docs/generate_llms_full.sh
  why: Defines EXACTLY which 8 files are concatenated into llms_full.txt and in
        what order, plus the front-matter stripping logic.
  pattern: |
    Concatenation order (emit()):
      1. README.md            (repo root — no Jekyll front matter)
      2. docs/index.md        (Home)
      3. docs/installation.md (Installation)   ← updated by P1.M6.T1.S1
      4. docs/qmk-integration.md
      5. docs/configuration.md
      6. docs/usage.md
      7. docs/examples.md
      8. docs/troubleshooting.md
    strip_fm() removes a leading `--- ... ---` Jekyll front-matter block;
    files without front matter (e.g. README.md) pass through whole.
  critical: |
    - The script does NOT (and must NOT, for this task) include spec/*.md or
      packaging/**/README.md — llms_full.txt is a USER-docs bundle by design.
      Do NOT "improve" the script by adding spec/packaging content here.
    - Output path is hard-coded: $DOCS_DIR/llms_full.txt (git-tracked).
    - `set -euo pipefail` — any error aborts. It prints
      `wrote .../llms_full.txt (<lines> lines, <bytes> bytes)` on success.
    - README.md at repo root has NO front matter; docs/*.md that start with
      `---` have it stripped. This is correct/intended.

# MUST READ — the spec to verify against (NOT to be edited here)
- file: spec/PACKAGING.md
  why: The PACKAGING spec referenced by PRD h2.72–h2.80. Its §6 (F15 channels)
        and §4.2/§4.5 (AUR/Nix) are the claims to verify against packaging/.
  sections:
    - "§3 Windows Packaging (Inno/install.ps1/WiX/runtime deps)"
    - "§4.1 Arch source PKGBUILD — packaging/linux/arch/"
    - "§4.2 AUR (qmkonnect-bin) — packaging/linux/aur/    [COMPLETE channel]"
    - "§4.3 .deb via cargo-deb — packaging/debian/         [— NEW, P1.M7, ABSENT]"
    - "§4.4 .rpm via cargo-generate-rpm — packaging/rpm/   [— NEW, P1.M7, ABSENT]"
    - "§4.5 Nix flake (flake.nix, repo root)               [COMPLETE channel]"
    - "§4.7 XDG autostart .desktop — packaging/linux/xdg/  [— NEW, P2.M6, ABSENT]"
    - "§6.1 Homebrew Cask — packaging/homebrew/Casks/qmkonnect.rb  [COMPLETE]"
    - "§6.2 Scoop — packaging/scoop/qmkonnect.json                 [COMPLETE]"
    - "§6.3 Winget — packaging/winget/*.yaml (3 files)             [COMPLETE]"
    - "§6.4 mise + asdf — packaging/asdf/                          [COMPLETE]"
  critical: |
    The spec intentionally describes TARGET end-state. §4.3/.deb, §4.4/.rpm,
    §4.7/.desktop are marked "— NEW" and their directories DO NOT EXIST yet
    (owned by P1.M7.T1/T2 and P2.M6.T1). Do NOT report these as "missing" or
    edit the spec to delete them — they are planned future work, not drift.

# MUST READ — the architecture context note cited in the contract
- file: plan/007_fb356ba503b4/architecture/system_context.md
  why: Confirms llms_full.txt is a committed 111KB aggregate; lists the F15
        channels as the active remaining work and the "Documentation State".

# READ — the packaging internal docs that cross-link to the spec (verify/fix these)
- file: packaging/winget/README.md
  why: Contains a STALE forward-reference on the "Install docs" cross-link line:
        "(Windows section — a Winget row is added in P1.M6.T1.S1)".
  fix: P1.M6.T1.S1 is now COMPLETE. docs/installation.md already has the Winget
        row. Remove/rewrite the parenthetical to present tense (e.g. "(Windows
        section — Winget row)" or drop the parenthetical entirely).
- file: packaging/scoop/README.md
  why: "Cross-links" section cites `spec/PACKAGING.md §3 (Windows packaging)`.
  note: §3 is the Inno/install.ps1 section — correct for the *underlying
        installer* Scoop wraps. Optionally also cite §6.2 (the Scoop channel
        itself). §3 reference is NOT wrong; do not break it.
- file: packaging/scoop/bucket-README.md
  why: Same §3 citation (absolute GitHub URL form). Same note as above.
- file: packaging/linux/aur/README.md
  why: Cites `spec/PACKAGING.md §4` (Linux Packaging) — CORRECT (AUR = §4.2).
        No change needed unless you want a precise §4.2 anchor.
```

### Current Codebase Tree (relevant slice)

```bash
docs/
├── generate_llms_full.sh   # THE script — concatenates 8 files into llms_full.txt
├── llms_full.txt           # 111KB, STALE (Aug 5) — predates README/installation updates (Aug 7)
├── installation.md         # updated Aug 7 (P1.M6.T1.S1) — community channels added
├── index.md, configuration.md, qmk-integration.md, usage.md, examples.md, troubleshooting.md
README.md                   # repo root — "Package Managers" section added Aug 7 (P1.M6.T2.S1)
spec/PACKAGING.md           # the spec to verify against (§6 = F15 channels)
flake.nix                   # repo root — Nix flake (spec §4.5 says "repo root", CONFIRMED present)
packaging/
├── homebrew/Casks/qmkonnect.rb   # §6.1 ✓
├── scoop/qmkonnect.json          # §6.2 ✓
├── winget/*.yaml (3)             # §6.3 ✓
├── asdf/                         # §6.4 ✓
├── linux/aur/                    # §4.2 ✓
├── nix/README.md                 # §4.5 detail doc ✓
├── linux/arch/                   # §4.1 ✓ (pre-existing source PKGBUILD)
├── (NO debian/)                  # §4.3 — NEW, P1.M7, absent by design
├── (NO rpm/)                     # §4.4 — NEW, P1.M7, absent by design
└── (NO linux/xdg/)               # §4.7 — NEW, P2.M6, absent by design
```

### Desired Codebase Tree (files touched)

```bash
docs/llms_full.txt          # REGENERATED (the primary deliverable)
packaging/winget/README.md  # EDITED — one stale forward-reference line fixed
# (packaging/scoop/{README,bucket-README}.md, packaging/linux/aur/README.md —
#  optional §6.2/§4.2 anchor tightening only; leave §3/§4 refs intact)
# NO other files change. spec/PACKAGING.md is READ-ONLY for this task.
```

### Known Gotchas of our Codebase & Library Quirks

```bash
# CRITICAL: docs/llms_full.txt IS git-tracked (confirmed: git ls-files lists it;
# docs/.gitignore only ignores Jekyll _site/.bundle/vendor). So regeneration
# produces a REAL, committable diff — verify with `git diff --stat`.

# CRITICAL: The regeneration script must be invoked from the docs/ dir per the
# contract: `cd docs && ./generate_llms_full.sh`. It computes ROOT/DOCS_DIR from
# $BASH_SOURCE so it also works from repo root (`bash docs/generate_llms_full.sh`),
# but follow the documented invocation to match AGENTS.md / the dev loop.

# GOTCHA: The script concatenates in a FIXED order with `emit "<n>. <file>"`
# headers wrapped in 80 '=' dividers. Do NOT hand-edit llms_full.txt — always
# regenerate; the header banner ("QMKonnect - Complete Documentation ...") is
# emitted from the heredoc at the top of the script.

# GOTCHA: strip_fm only strips a LEADING front-matter block (line 1 == '---').
# Mid-file '---' (e.g. YAML doc-separators inside content) is preserved. This is
# correct; do not "fix" it.

# GOTCHA (scope guard): spec/PACKAGING.md §4.3 (.deb), §4.4 (.rpm), §4.7 (.desktop)
# describe DIRECTORIES THAT DO NOT EXIST. They are labeled "— NEW" and belong to
# P1.M7 / P2.M6. Do NOT treat their absence as a defect and do NOT edit the spec
# to remove them. Only verify the COMPLETED channels (AUR, Nix, Homebrew, Scoop,
# Winget, asdf/mise).

# GOTCHA (test env): this task changes ONLY .md/.txt — no .rs. cargo test is a
# pure regression guard and should pass identically to the prior green run. If
# the host lacks Linux build deps for a full build, `cargo check --bin qmkonnect`
# is the acceptable fallback compile-gate (the contract asks for the test run;
# prefer the full `cargo test` and only fall back if the toolchain/deps are
# unavailable, documenting why).
```

## Implementation Blueprint

### Data models and structure

_None._ This is a documentation task — no types, schemas, or runtime models.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: REGENERATE docs/llms_full.txt
  - RUN: cd docs && ./generate_llms_full.sh
  - EXPECT: exit 0; final line "wrote .../llms_full.txt (<lines> lines, <bytes> bytes)"
  - VERIFY fresh build:
      1. test docs/llms_full.txt -nt README.md     # mtime newer than the newest input
      2. grep -Eic 'homebrew|scoop|winget|brew install|nix run|asdf|aur' docs/llms_full.txt
         # MUST rise from the stale count of 1 to >=8 (README.md alone has ~10)
      3. git diff --stat docs/llms_full.txt        # non-empty, content-bearing
  - VERIFY the bundle integrity is intact (sanity, not a full read):
      grep -c 'Complete Documentation' docs/llms_full.txt   # == 1 (the header banner)
  - DEPENDENCIES: README.md (P1.M6.T2.S1) + docs/installation.md (P1.M6.T1.S1)
        already committed — these are the INPUTS this task consumes.
  - NAMING/PLACEMENT: output stays at docs/llms_full.txt (hard-coded in script).

Task 2: VERIFY spec/PACKAGING.md F15 coverage ↔ packaging/ structure
  - RUN this mapping check and record results (every row must RESOLVE):
      | spec claim                         | expected path                              | status      |
      | §6.1 Homebrew Cask                 | packaging/homebrew/Casks/qmkonnect.rb      | must EXIST  |
      | §6.2 Scoop                         | packaging/scoop/qmkonnect.json             | must EXIST  |
      | §6.3 Winget                        | packaging/winget/*.yaml (>=1 file)         | must EXIST  |
      | §6.4 mise + asdf                   | packaging/asdf/ (bin/{install,download,list-all}) | must EXIST |
      | §4.2 AUR (qmkonnect-bin)           | packaging/linux/aur/ (PKGBUILD+.SRCINFO)   | must EXIST  |
      | §4.5 Nix flake                     | flake.nix (repo ROOT, not packaging/)      | must EXIST  |
      | §4.1 Arch source PKGBUILD          | packaging/linux/arch/PKGBUILD              | must EXIST  |
    One-liner to assert all at once:
      for p in packaging/homebrew/Casks/qmkonnect.rb packaging/scoop/qmkonnect.json \
               packaging/winget/dabstractor.QMKonnect.yaml packaging/asdf/bin/install \
               packaging/linux/aur/PKGBUILD flake.nix packaging/linux/arch/PKGBUILD ; \
        do test -e "$p" && echo "OK  $p" || echo "MISSING $p"; done
      # Expected: all OK. Any MISSING here is a real defect to surface.
  - DO NOT flag as missing (future state, out of scope): packaging/debian/
      (§4.3, P1.M7.T1), packaging/rpm/ (§4.4, P1.M7.T2), packaging/linux/xdg/
      (§4.7, P2.M6.T1).
  - OUTPUT: a short verification note (can live in the commit message and/or
      plan/007_fb356ba503b4/P1M6T2S2/research/) confirming each row resolved.

Task 3: FIX stale forward-reference in packaging/winget/README.md
  - FIND (exact current text, the "Install docs" cross-link, ~line 154):
      - **Install docs:** [`docs/installation.md`](../../docs/installation.md)
        (Windows section — a Winget row is added in P1.M6.T1.S1)
  - REPLACE the parenthetical — P1.M6.T1.S1 is COMPLETE and the Winget row now
      exists in docs/installation.md. Suggested present-tense wording:
      (Windows section — Winget row)
    or simply drop the parenthetical. Keep the markdown link to installation.md.
  - FOLLOW pattern: the sibling packaging/scoop/README.md "Install docs" line,
      which has no pending-work qualifier (model the winget line on it).
  - DEPENDENCIES: Task 2 confirms installation.md content is current.

Task 4 (OPTIONAL, low-risk): tighten spec cross-link anchors in packaging docs
  - SCOPE: only if you judge it improves accuracy; do NOT break existing refs.
  - packaging/scoop/README.md + bucket-README.md cite spec/PACKAGING.md §3
      (Windows packaging) — this is CORRECT for the Inno installer Scoop wraps.
      Optionally append the channel's own section, e.g. "§3 (Inno installer) and
      §6.2 (this channel)". Leave §3 anchor in place either way.
  - packaging/linux/aur/README.md cites §4 — optionally narrow to §4.2.
  - If you make NO change here, that is acceptable; the §3/§4 refs are not wrong.
  - DO NOT edit spec/PACKAGING.md itself (READ-ONLY — spec is the source of truth).

Task 5: REGRESSION-TEST
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
  - WHY: contract requirement; this task changed only .md/.txt so a green run
      identical to the prior baseline confirms no accidental source/script edits.
  - EXPECT: all tests pass (single-threaded because the debouncer uses shared
      global state — see AGENTS.md).
  - FALLBACK (only if host lacks Linux build deps): cargo check --bin qmkonnect
      must compile cleanly; record the reason the full test could not run.
```

### Implementation Patterns & Key Details

```bash
# The one canonical regeneration + verification sequence (run from repo root):
set -e
cd docs && ./generate_llms_full.sh && cd ..
# 1. freshness: bundle newer than newest input
test docs/llms_full.txt -nt README.md && echo "freshness OK"
# 2. content: community channels now present (stale baseline was 1)
n=$(grep -Eic 'homebrew|scoop|winget|brew install|nix run|asdf|aur' docs/llms_full.txt)
[ "$n" -ge 8 ] && echo "content OK ($n community-channel mentions)"
# 3. integrity: exactly one header banner
[ "$(grep -c 'Complete Documentation' docs/llms_full.txt)" -eq 1 ] && echo "banner OK"
# 4. diff is real
git diff --stat docs/llms_full.txt
```

### Integration Points

```yaml
DOCUMENTATION (no code/config/CI integration):
  - regenerate: docs/llms_full.txt        # via docs/generate_llms_full.sh
  - edit: packaging/winget/README.md      # stale forward-ref → present tense
  - read-only verify: spec/PACKAGING.md   # NOT modified by this task

NO CHANGES TO:
  - Cargo.toml, any .rs, .github/workflows/*, release.toml, .cargo/config.toml
  - spec/PACKAGING.md (source of truth; owned across the F15/F16/F17 plan)
  - docs/generate_llms_full.sh (it already does exactly what's needed)
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Markdown sanity on any packaging README you edited (Task 3/4):
#   - links still resolve (no dangling relative paths)
#   - the spec cross-link target file exists
test -f spec/PACKAGING.md && echo "spec link target OK"

# No linter is configured for Markdown in this repo; rely on visual + link checks.
# Expected: edited READMEs render; relative links (../../spec/PACKAGING.md) resolve.
```

### Level 2: Regeneration Correctness (the core gate)

```bash
# Regenerate + assert freshness/content/integrity (run from repo root):
cd docs && ./generate_llms_full.sh && cd ..
test docs/llms_full.txt -nt README.md                                            && echo "PASS freshness"
n=$(grep -Eic 'homebrew|scoop|winget|brew install|nix run|asdf|aur' docs/llms_full.txt); [ "$n" -ge 8 ] && echo "PASS content ($n)"
[ "$(grep -c 'Complete Documentation' docs/llms_full.txt)" -eq 1 ]                && echo "PASS banner"
git diff --stat docs/llms_full.txt                                                # non-empty
# Expected: all three PASS lines; a non-empty, content-bearing diff.
```

### Level 3: Spec ↔ Structure Verification (Task 2)

```bash
# Every COMPLETED F15 channel claim must resolve to a real path:
for p in \
  packaging/homebrew/Casks/qmkonnect.rb \
  packaging/scoop/qmkonnect.json \
  packaging/winget/dabstractor.QMKonnect.yaml \
  packaging/asdf/bin/install \
  packaging/linux/aur/PKGBUILD \
  flake.nix \
  packaging/linux/arch/PKGBUILD ; do
  test -e "$p" && echo "OK       $p" || echo "MISSING  $p"
done
# Expected: all OK. (debian/rpm/xdg are intentionally absent — P1.M7/P2.M6.)
```

### Level 4: Regression Test (Task 5)

```bash
cargo test --bin qmkonnect -- --test-threads=1
# Expected: all tests pass (no .rs changed; this is a regression guard).
# Single-threaded is mandatory: shared global debouncer state (AGENTS.md).
```

## Final Validation Checklist

### Technical Validation
- [ ] Level 2 (regeneration): freshness + content + banner all PASS.
- [ ] Level 3 (spec↔structure): all completed-channel paths OK; no MISSING.
- [ ] Level 4 (regression): `cargo test --bin qmkonnect -- --test-threads=1` green.
- [ ] `git diff --stat docs/llms_full.txt` is non-empty and content-bearing.
- [ ] `git diff --stat packaging/winget/README.md` shows the forward-ref fix.

### Feature Validation
- [ ] All success criteria from "What" section met.
- [ ] The spec's future-state `— NEW` sections (§4.3/.deb, §4.4/.rpm, §4.7/.desktop)
      were NOT edited and NOT reported as defects.
- [ ] No `spec/PACKAGING.md` edits (read-only for this task).
- [ ] No `docs/generate_llms_full.sh` edits (already correct).
- [ ] The winget README no longer says the install-docs row "is added in P1.M6.T1.S1".

### Code Quality Validation
- [ ] Edited packaging READMEs keep relative-link targets valid
      (`../../spec/PACKAGING.md`, `../../docs/installation.md`).
- [ ] Only documentation files touched — no source/config/CI changes.
- [ ] Verification note (spec↔structure table) recorded in commit message or
      `plan/007_fb356ba503b4/P1M6T2S2/research/`.

### Documentation & Deployment
- [ ] `docs/llms_full.txt` mtime > newest input (README.md / docs/installation.md).
- [ ] Community-channel strings present in the regenerated bundle.
- [ ] Commit message describes what was regenerated + which spec claims verified.

---

## Anti-Patterns to Avoid

- ❌ Don't hand-edit `docs/llms_full.txt` — always regenerate via the script.
- ❌ Don't add `spec/` or `packaging/**` content to the script/`llms_full.txt`
      here — the bundle is intentionally user-docs-only.
- ❌ Don't report `packaging/debian/`, `packaging/rpm/`, or
      `packaging/linux/xdg/` as missing defects — they are planned (P1.M7/P2.M6).
- ❌ Don't edit `spec/PACKAGING.md` — it is the source of truth, read-only here.
- ❌ Don't change the existing correct `§3` (Windows) / `§4` (Linux) cross-links
      in scoop/winget/aur READMEs; only optionally augment with the channel's own
      section (§6.2/§6.3/§4.2).
- ❌ Don't skip `cargo test` because "it's just docs" — it's the contract's
      regression gate; run it (fall back to `cargo check` only if deps absent,
      with a documented reason).