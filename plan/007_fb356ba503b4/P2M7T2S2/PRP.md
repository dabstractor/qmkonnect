# PRP — P2.M7.T2.S2: Regenerate `docs/llms_full.txt` + final build verification

> **PRD context:** PRD §4 F16/F17 (cross-DE Linux window monitor + universal
> autostart), §5 Supported Platforms (Compatibility Matrix), and the
> packaging/config rows. This is the **final verification gate** for the entire
> P2 milestone (F16/F17): it regenerates the single-file agent docs bundle from
> the now-updated user-facing markdown, confirms the new F16 backends + config
> introduce no test regressions or clippy warnings, and traces the PRD §5
> compatibility claims back to compiled-in backends.
>
> **This is a RUNBOOK task** (run commands + verify + commit a regenerated
> artifact), not a feature-build task. There is no new Rust, no new schemas, no
> migrations. The deliverable is: a regenerated `docs/llms_full.txt`, green
> `cargo test`, green `cargo clippy --all-targets -- -D warnings`, and a
> documented §5→backend traceability check.

---

## Goal

**Feature Goal**: After every P2 source change (the five F16 backends in
`src/platforms/`, the `[linux]` config table, XDG `.desktop`) and every P2 doc
change (P2.M7.T1.S1's README/installation/troubleshooting rewrites), regenerate
the canonical single-file agent-docs bundle `docs/llms_full.txt`, then prove the
milestone introduces no regressions and that the PRD §5 platform-coverage claims
are each backed by a real, compiled-in backend.

**Deliverable**:
1. Regenerated `docs/llms_full.txt` (a real `git diff` against the committed
   copy — it is git-tracked, not gitignored).
2. A green `cargo test --bin qmkonnect -- --test-threads=1` run.
3. A green `cargo clippy --all-targets -- -D warnings` run.
4. A documented PRD §5 (GNOME/KDE Plasma/COSMIC/Hyprland/Sway/Niri/wlroots/X11)
   → backend traceability table (confirm-and-record; no test exists for it).

**Success Definition**:
- `bash docs/generate_llms_full.sh` (or `cd docs && ./generate_llms_full.sh`)
  exits 0 and `git diff --stat docs/llms_full.txt` shows the file changed (or is
  clean if inputs were already captured — both are acceptable; **clean = the
  prior regen already captured the edits**).
- `cargo test --bin qmkonnect -- --test-threads=1` → `test result: ok. … 0
  failed`.
- `cargo clippy --all-targets -- -D warnings` → exits 0, no warnings.
- Every DE named in PRD §5 maps to a backend with a row in
  `src/platforms/linux.rs::linux_backend_candidates()` and an arm in
  `construct_backend()` (verified by grep), recorded below in §Implementation.

---

## User Persona (if applicable)

**Target User**: AI agents / LLMs that consume `docs/llms_full.txt` as the
single-file canonical reference for the project (it is linked as such). Secondary:
the release/milestone maintainer who needs proof the P2 milestone is green before
tagging.

**Use Case**: An agent opens `docs/llms_full.txt` to learn QMKonnect end-to-end
without crawling the repo; the bundle must reflect the post-F16 cross-DE reality,
not the stale "Linux (Hyprland only)" text.

**User Journey**: P2.M7.T1.S1 rewrites the user-facing markdown → THIS task
re-runs the generator → the bundle is regenerated and committed → tests + clippy
prove the milestone source is sound → §5 traceability proves the docs' coverage
claims are real.

**Pain Points Addressed**: stale agent docs that contradict the shipped cross-DE
support; an undetected clippy/test regression from the new backends; an
unsupported-DE claim in the PRD with no backing backend.

---

## Why

- **Agent/LLM accuracy:** `docs/llms_full.txt` is the *single-file* doc bundle
  piped to agents. If it still says "Hyprland only" after F16 shipped, every
  agent consuming it will give wrong Linux advice. Regeneration is the only thing
  that fixes this — the bundle is a generated artifact, not hand-edited.
- **Milestone gate integrity:** P2 added ~5 large backend files
  (`wayland_ft.rs` 27 KB, `gnome.rs` 16 KB, `atspi.rs` 25 KB) + config plumbing.
  The single-threaded test suite + clippy `-D warnings` is the deterministic
  proof the milestone didn't regress the build. `--test-threads=1` is mandatory
  because the notifier/debouncer shares global mutable state (AGENTS.md, both OS
  loops) — parallel tests race.
- **Truth-in-advertising:** PRD §5 advertises broad Linux DE coverage. This task
  is the explicit check that each advertised DE has a compiled-in backend, so the
  docs and the binary agree.

---

## What

### User-visible behavior
- `docs/llms_full.txt` content matches the current README + `docs/*.md`
  (cross-DE Linux story present; no stale "Hyprland only" text in the bundle).
- The P2 milestone build is green: tests pass single-threaded, clippy is warning-
  free across all targets.
- A maintainer can read the §5→backend mapping recorded by this task and trust
  that "supported" means "a backend exists."

### Success Criteria
- [ ] `docs/llms_full.txt` regenerated via `generate_llms_full.sh`; the stale
      "Hyprland only" string is **absent** from the regenerated bundle.
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → 0 failed.
- [ ] `cargo clippy --all-targets -- -D warnings` → exit 0.
- [ ] §5 traceability table populated; every §5 DE has a matching backend row.
- [ ] `git diff --stat` for this task lists `docs/llms_full.txt` (+ any clippy
      fix in `src/`), **never** `PRD.md`, `tasks.json`, `prd_snapshot.md`, or
      `spec/*.md`.

---

## All Needed Context

### Context Completeness Check
_If someone knew nothing about this codebase, would they have everything needed
to implement this successfully?_ **Yes** — exact commands, the script's real
behavior (with the contract correction), the mandatory `--test-threads=1`
rationale, the host-OS clippy caveat, the input-file list, and the full §5→backend
mapping with grep checks are all below.

### Documentation & References

```yaml
# MUST READ — the generator script (THE thing this task runs).
- file: docs/generate_llms_full.sh
  why: "The actual generator. NOTE: its header comment and the work-item
        contract say it 'concatenates spec docs + README + source' — that is
        INACCURATE. It concatenates ONLY README.md + 7 docs/*.md files
        (index/installation/qmk-integration/configuration/usage/examples/
        troubleshooting). strip_fm() drops leading Jekyll front matter. It is
        idempotent + deterministic + OS-independent (pure bash/awk/cat), runs
        from anywhere via BASH_SOURCE, and is executable (mode 0755)."
  pattern: "emit() number+path banner, strip_fm() per file, single redirect to OUT"
  gotcha: "It does NOT ingest spec/*.md or src/**/*.rs, so source/spec edits do
           NOT by themselves require a regen — only README.md + docs/*.md edits do."

# MUST READ — what the sibling doc task changed (the reason for this regen).
- file: plan/007_fb356ba503b4/P2M7T1S1/PRP.md
  why: "P2.M7.T1.S1 (status: Ready) MODIFIES README.md, docs/installation.md,
        docs/troubleshooting.md, and optionally docs/configuration.md — ALL of
        which are llms_full.txt inputs. Its Success Definition mandates the stale
        strings 'hyprland only' / 'not supported yet' be GONE from those files.
        THIS task's regen therefore captures that rewrite. If those files still
        contain the stale strings, P2.M7.T1.S1 hasn't landed yet — regenerate
        anyway (harmless) but flag it."
  pattern: "Stale-string purge + cross-DE narrative in the 3-4 input files"

# MUST READ — the PRD §5 matrix this task verifies.
- file: spec/PRD.md
  why: "§5 'Supported Platforms (Compatibility Matrix)' (~L154): the Linux row
        claims GNOME / KDE Plasma / COSMIC / Hyprland / Sway / Niri / wlroots
        (Wayland) + XFCE / MATE / Cinnamon / Budgie / LXQt (X11). This task
        traces each to a backend. spec/PRD.md is READ-ONLY — never edit it."
  section: "## 5. Supported Platforms (Compatibility Matrix) (line ~154)"

# MUST READ — the backend candidate list + constructor (the traceability source).
- file: src/platforms/linux.rs
  why: "linux_backend_candidates() (priority-ordered list) + construct_backend()
        (the match arms) are the SINGLE source of truth for which backends exist.
        Every candidate needs BOTH a row here AND an arm to be real. X11 is
        unconditional and always LAST (lowest priority)."
  section: "fn linux_backend_candidates (~L42); fn construct_backend (~L72)"

# MUST READ — the feature flags that gate the backends.
- file: Cargo.toml
  why: "[features] default = ['wayland','gnome','atspi','hyprland','macos',
        'linux-tray'] — so a plain build compiles ALL Linux backends (x11 is
        unconditional). Each backend maps to a dep (smithay-client-toolkit/zbus/
        atspi/hyprland). Confirms §5 coverage is on-by-default, not opt-in."
  section: "[features] (line ~125); [target.'cfg(target_os = \"linux\")'] (~L46)"

# MUST READ — the mandatory test/threading + dev-loop conventions.
- file: AGENTS.md
  why: "Both the macOS and Windows dev loops specify the EXACT test command:
        'cargo test --bin qmkonnect -- --test-threads=1' — single-threaded because
        the notifier/debouncer shares global mutable state. Dropping the flag
        makes tests flake. This task uses the identical command."
  pattern: "cargo test --bin qmkonnect -- --test-threads=1"

# REFERENCE — the platform backend-selection spec (mapping rationale).
- file: spec/PLATFORMS.md
  why: "§6 'Linux Backend Selection' documents the priority order + per-DE
        coverage rationale that backs the §5 matrix. §7-§10 detail each backend."
  section: "## 6. Linux Backend Selection (~L286); §7 wayland; §8 gnome; §9 atspi; §10 x11"

# REFERENCE — llms_full.txt IS tracked (so regen = a commit).
- file: docs/llms_full.txt
  why: "Current committed bundle (3014 lines, ~119 KB). git ls-files confirms it
        is tracked; docs/.gitignore only covers Jekyll _site/.bundle/vendor. The
        regenerated diff is part of the deliverable."
  gotcha: "A CLEAN diff (no change) is ALSO acceptable — it means a prior regen
           already captured the edits. Do not force a change."
```

### Current Codebase tree (relevant slice)

```bash
docs/
  generate_llms_full.sh    # RUN — the generator (idempotent, deterministic)
  llms_full.txt            # OUTPUT (git-tracked) — regenerated by the script
  index.md                 # INPUT #2
  installation.md          # INPUT #3  (rewritten by P2.M7.T1.S1)
  qmk-integration.md       # INPUT #4
  configuration.md         # INPUT #5  (lightly edited by P2.M7.T1.S1)
  usage.md                 # INPUT #6
  examples.md              # INPUT #7
  troubleshooting.md       # INPUT #8  (rewritten by P2.M7.T1.S1)
  .gitignore               # READ — confirms llms_full.txt is NOT ignored
README.md                  # INPUT #1  (edited by P2.M7.T1.S1)
spec/PRD.md                # READ-ONLY — §5 matrix this task verifies
spec/PLATFORMS.md          # READ — §6 backend-selection rationale
src/platforms/linux.rs     # READ — linux_backend_candidates + construct_backend
src/platforms/{wayland_ft,gnome,atspi,hyprland,x11}.rs  # the F16 backends (clippy-exercised on Linux)
Cargo.toml                 # READ — [features] default backend set
AGENTS.md                  # READ — mandatory --test-threads=1 convention
```

### Desired Codebase tree after this task

```bash
docs/llms_full.txt   # REGENERATED (committed diff) — the only expected artifact
# POSSIBLY: behavior-preserving clippy fixes in src/platforms/*.rs IF clippy
#   flags the new F16 backend code. Otherwise NO source changes.
# NEVER: PRD.md, spec/*.md, tasks.json, prd_snapshot.md, .gitignore, packaging/*,
#   .github/workflows/* (those are owned by P2.M7.T2.S1 / humans).
```

### Known Gotchas of our codebase & Library Quirks

```yaml
# CRITICAL — the generator's real behavior ≠ the contract wording. It ingests
# ONLY README.md + 7 docs/*.md. It does NOT pull spec/*.md or src/. So this task
# does NOT need to run after a pure SOURCE change — only after a README/docs
# markdown change. (P2.M7.T1.S1 changed exactly those, so the regen IS needed.)

# CRITICAL — --test-threads=1 is MANDATORY, not optional. The notifier/debouncer
# shares global mutable state (once_cell statics); parallel tests race and flake.
# Both AGENTS.md dev loops specify this exact flag. Dropping it = false failures.

# CRITICAL — clippy --all-targets -D warnings checks TEST modules too (the 463
# #[test] fns live in inline #[cfg(test)] modules). There is NO [lints] table /
# clippy config / #![deny(warnings)] in the project, so the explicit -D warnings
# on the command line IS the gate. If the new F16 backend code lints, FIX it
# (behavior-preserving) — that is in-scope for this final milestone gate.

# GOTCHA — host OS determines which backend code clippy sees. wayland_ft/gnome/
# atspi are #[cfg(target_os="linux")]-gated → only checked ON LINUX. On macOS/
# Windows they are cfg'd out. IDEALLY run clippy on Linux to exercise the new
# backends; the per-OS ci.yml covers the other hosts. If only a non-Linux host
# is available, note the gap and rely on CI for Linux-side coverage.

# GOTCHA — docs/llms_full.txt is git-tracked, so regeneration = a real diff to
# commit. A CLEAN diff is fine (prior regen already captured edits); do NOT
# fabricate a change. The diff content must reflect ONLY markdown edits, never
# structural generator changes (the script is stable).

# GOTCHA — the script uses set -euo pipefail; if ANY of the 8 input files is
# missing it aborts. All 8 exist today; this only matters if someone deletes one.
```

---

## Implementation Blueprint

### Data models and structure
None — this is a runbook/verification task. No new types, no schemas.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 0: CONFIRM prerequisites (P2.M7.T1.S1 docs landed; P2 source present)
  - CHECK the stale-string purge from P2.M7.T1.S1 actually happened in the
    llms_full.txt INPUT files (NOT yet the bundle):
      grep -rinE 'hyprland only|only supports hyprland|not supported yet|other window managers are not supported' \
        README.md docs/installation.md docs/troubleshooting.md docs/configuration.md
    Expected: ZERO hits (P2.M7.T1.S1 removed them). If hits remain, P2.M7.T1.S1
    has NOT landed — note it, then proceed (the regen is idempotent and will
    capture whatever the inputs currently say; a later re-run after T1.S1 lands
    is the correct fix).
  - CHECK the F16 backend source exists (the thing clippy will lint):
      ls src/platforms/wayland_ft.rs src/platforms/gnome.rs src/platforms/atspi.rs \
         src/platforms/hyprland.rs src/platforms/x11.rs src/platforms/linux.rs
    Expected: all six present (P2.M2-P2.M5 delivered them).
  - DO NOT edit anything in Task 0 — it is read-only confirmation.

Task 1: REGENERATE docs/llms_full.txt
  - RUN the generator (either form works — it locates its own dir via BASH_SOURCE):
      cd docs && ./generate_llms_full.sh
    # OR from repo root:  bash docs/generate_llms_full.sh
  - EXPECT stdout: "wrote .../docs/llms_full.txt (<N> lines, <M> bytes)".
  - VERIFY the regenerated bundle no longer carries the stale Linux claim:
      grep -ic 'hyprland only' docs/llms_full.txt   # expect 0
      grep -ic 'not supported yet' docs/llms_full.txt  # expect 0
  - VERIFY the regenerated bundle DOES carry the cross-DE story:
      grep -icE 'foreign-toplevel|backend selection|select_linux_backend|gnome shell extension|atspi' \
        docs/llms_full.txt   # expect >=1 (P2.M7.T1.S1 added these)
  - INSPECT the diff scope (content change only, no structural churn):
      git diff --stat docs/llms_full.txt
    A non-empty diff is EXPECTED (the bundle was ~3014 lines of stale text). A
    CLEAN diff is ALSO acceptable (means a prior run already captured the edits).
  - DO NOT hand-edit docs/llms_full.txt — it is a generated artifact. Re-run the
    script if it's wrong.

Task 2: RUN the test suite (no-regression gate) — MANDATORY single-threaded
  - RUN exactly:
      cargo test --bin qmkonnect -- --test-threads=1
  - RATIONALE: AGENTS.md (both macOS + Windows loops) mandate --test-threads=1
    because the notifier/debouncer shares global mutable state (once_cell
    statics); parallel tests race. ~463 #[test] fns across src/core/* +
    src/platforms/*.
  - EXPECT: "test result: ok. <N> passed; 0 failed; …" and exit 0.
  - IF a test FAILS: read the failure, identify the root cause in the F16 backend
    or config code (src/platforms/*.rs, src/core/mod.rs [linux] table), and fix
    it (behavior-preserving where possible). This is the milestone's regression
    gate — a failure here blocks P2. Re-run until green.

Task 3: RUN clippy (the -D warnings gate) — all targets
  - RUN exactly:
      cargo clippy --all-targets -- -D warnings
  - RATIONALE: no [lints] table / clippy config exists, so -D warnings on the CLI
    IS the gate. --all-targets also lints the inline #[cfg(test)] modules.
  - HOST-OS NOTE: the F16 backends are #[cfg(target_os="linux")]-gated, so only a
    LINUX host exercises wayland_ft/gnome/atspi via clippy. Prefer Linux. On a
    non-Linux host, the Linux backend code is cfg'd out — note the gap and rely
    on ci.yml's Linux job for that coverage.
  - EXPECT: exit 0, "Finished" with NO warning lines.
  - IF clippy flags a WARNING in the new F16 backend source (wayland_ft.rs /
    gnome.rs / atspi.rs) or anywhere else: FIX it (behavior-preserving — e.g.
    unused imports, needless_borrow, needless_return, redundant clone, clippy::
    correctnes/style lints). Do NOT change observable behavior. Re-run until
    green. This is in-scope: the task's OUTPUT is "passing clippy."

Task 4: VERIFY the PRD §5 compatibility matrix is backed by backends (traceability)
  - This is a STATIC confirm-and-document check — there is no automated test.
  - READ the matrix (READ-ONLY, never edit spec/PRD.md):
      sed -n '/## 5\. Supported Platforms/,/^---/p' spec/PRD.md
  - EXTRACT the compiled-in backends from the source of truth:
      grep -nE '^\s*\("#?[a-z-]+",|^\s*"[a-z-]+" =>|feature = "' src/platforms/linux.rs
    Confirms each candidate has BOTH a linux_backend_candidates() row AND a
    construct_backend() arm.
  - EXTRACT the feature flags that gate them:
      sed -n '/\[features\]/,/^\[/p' Cargo.toml
  - POPULATE the §5→backend mapping (record in the task's validation output):
      | PRD §5 DE                              | Backed by backend (priority)            |
      | GNOME                                  | gnome (#2)                              |
      | KDE Plasma                             | foreign-toplevel wayland (#1)           |
      | COSMIC                                 | foreign-toplevel wayland (#1)           |
      | Hyprland                               | foreign-toplevel wayland (#1) + hyprland IPC (#3, legacy) |
      | Sway                                   | foreign-toplevel wayland (#1)           |
      | Niri                                   | foreign-toplevel wayland (#1)           |
      | wlroots family                         | foreign-toplevel wayland (#1)           |
      | X11 tail (XFCE/MATE/Cinnamon/Budgie/LXQt) | x11 (#5, unconditional)              |
  - EXPECT: all 8 §5 DEs have a backing backend (they do, per the verified
    priority list). default = ['wayland','gnome','atspi','hyprland','macos',
    'linux-tray'] means all Wayland/GNOME/Hyprland/AT-SPI backends compile by
    default; x11 is unconditional.
  - IF (hypothetically) a §5 claim were UNBACKED: do NOT edit spec/PRD.md
    (human-owned, read-only) — record the gap as a finding for the human. (Today
    all are backed, so this is confirm-only.)
  - OUTPUT: the completed mapping table is this task's traceability record.

Task 5: FINAL diff-hygiene check (what this task is allowed to change)
  - RUN:
      git diff --stat
      git status --porcelain
  - EXPECT changed files limited to:
      * docs/llms_full.txt                (the regenerated bundle — required)
      * src/**/*.rs (ONLY if Task 2/3 needed a behavior-preserving fix)
  - FORBIDDEN in this task's diff: PRD.md, spec/*.md, tasks.json,
    prd_snapshot.md, .gitignore, packaging/*, .github/workflows/*,
    Cargo.toml, docs/*.md (those are owned by P2.M7.T1.S1 / humans / siblings).
    If any appear, STOP — you've over-stepped scope.
```

### Implementation Patterns & Key Details

```bash
# 1. The regen is a single command; never hand-edit the bundle.
bash docs/generate_llms_full.sh && git diff --stat docs/llms_full.txt

# 2. Tests MUST be single-threaded (shared debouncer state — AGENTS.md).
cargo test --bin qmkonnect -- --test-threads=1

# 3. Clippy IS the lint gate (no [lints] table exists).
cargo clippy --all-targets -- -D warnings

# 4. The §5 verification is grep-driven traceability, not a test:
grep -nE '"[a-z-]+" =>|feature = "' src/platforms/linux.rs   # backends + arms
sed -n '/\[features\]/,/^\[/p' Cargo.toml                    # feature gating
sed -n '/## 5\. Supported Platforms/,/^---/p' spec/PRD.md    # the claims

# 5. Determinism check (optional, proves no churn): regen twice, expect no diff.
bash docs/generate_llms_full.sh
cp docs/llms_full.txt /tmp/llms_a.txt
bash docs/generate_llms_full.sh
diff /tmp/llms_a.txt docs/llms_full.txt   # expect: no output (identical)
```

### Integration Points

```yaml
DOCS BUNDLE (docs/llms_full.txt):
  - regenerate: "bash docs/generate_llms_full.sh  (or: cd docs && ./generate_llms_full.sh)"
  - inputs: "README.md + docs/{index,installation,qmk-integration,configuration,usage,examples,troubleshooting}.md"
  - tracking: "git-tracked (NOT gitignored) → regen produces a committed diff"

BUILD GATES:
  - test:    "cargo test --bin qmkonnect -- --test-threads=1   # MANDATORY single-thread"
  - clippy:  "cargo clippy --all-targets -- -D warnings        # the lint gate"

NO CONFIG/MIGRATION/MANIFEST CHANGES:
  - "This task changes no Cargo.toml, no flake.nix, no packaging, no CI YAML."
  - "Only docs/llms_full.txt (required) and, if needed, behavior-preserving
     clippy fixes in src/."
```

---

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# The regenerated bundle is plain text; the only "syntax" check is that the
# generator succeeded and the bundle is non-empty + structurally intact.
test -s docs/llms_full.txt && echo "bundle non-empty: OK"
grep -c '^ \([0-9]\+\.\) ' docs/llms_full.txt   # the 8 emit() section banners
# Expected: 8 (one per input file).

# Determinism re-check (no churn between runs).
bash docs/generate_llms_full.sh && cp docs/llms_full.txt /tmp/a.txt \
  && bash docs/generate_llms_full.sh && diff /tmp/a.txt docs/llms_full.txt
# Expected: diff produces no output.

# (Rust syntax/style is covered by Level 2/3 — clippy --all-targets -D warnings.)
```

### Level 2: Unit / Component Validation (the two build gates)

```bash
# GATE 1 — tests, MANDATORY single-threaded (AGENTS.md; shared debouncer state).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: "test result: ok." with 0 failed; exit 0.

# GATE 2 — clippy across ALL targets (incl. the inline test modules), warnings
# denied. There is no [lints]/clippy config in the project, so this CLI flag IS
# the gate.
cargo clippy --all-targets -- -D warnings
# Expected: exit 0, "Finished" with no warning lines.
# HOST NOTE: the F16 backends (wayland_ft/gnome/atspi) are cfg(linux)-gated, so
# only a Linux host lints them here; ci.yml covers the other hosts.
```

### Level 3: Integration / System Validation (the §5 traceability + diff scope)

```bash
# §5 matrix → backend traceability (confirm-and-document; no test exists).
echo "=== PRD §5 claims ==="
sed -n '/## 5\. Supported Platforms/,/^---/p' spec/PRD.md | grep -iE 'GNOME|KDE|COSMIC|Hyprland|Sway|Niri|wlroots|XFCE|MATE|Cinnamon|Budgie|LXQt'

echo "=== compiled-in backends (candidate rows + construct arms) ==="
grep -nE '"[a-z-]+"' src/platforms/linux.rs | grep -iE 'foreign-toplevel|gnome|hyprland|atspi|x11'

echo "=== feature gating ==="
sed -n '/\[features\]/,/^\[/p' Cargo.toml | grep -E '^(default|wayland|gnome|atspi|hyprland) '

# Record the populated §5→backend table (see Task 4). All 8 DEs must map.
# Expected: every §5 DE has a backing backend row + arm.

# Diff-hygiene: this task may change ONLY docs/llms_full.txt (+ optional clippy
# fixes in src/). Anything else = scope over-reach.
echo "=== changed files (scope check) ==="
git status --porcelain
# Expected: M docs/llms_full.txt   (and possibly M src/... if clippy was fixed)
# Forbidden: PRD.md, spec/*.md, tasks.json, prd_snapshot.md, .gitignore,
#   packaging/*, .github/workflows/*, Cargo.toml, docs/*.md
```

### Level 4: Creative & Domain-Specific Validation

```bash
# Bundle-content sanity: the regenerated bundle must reflect the F16 reality.
echo "=== stale-claim purge in the bundle ==="
grep -icE 'hyprland only|only supports hyprland|not supported yet|other window managers are not supported' \
  docs/llms_full.txt
# Expected: 0  (P2.M7.T1.S1 removed these from the input files; the regen
#                propagates the removal into the bundle.)

echo "=== cross-DE story present in the bundle ==="
grep -icE 'foreign-toplevel|backend (auto-)?selection|select_linux_backend|gnome shell extension|atspi' \
  docs/llms_full.txt
# Expected: >=1  (the new cross-DE narrative is now in the bundle.)

# Optional: bundle freshness vs inputs (catches a forgotten regen). Each input's
# mtime should be <= the bundle's mtime after Task 1.
stat -c '%Y %n' README.md docs/*.md docs/llms_full.txt | sort -n | tail -1
# Expected: docs/llms_full.txt is the newest (or tied) — confirms a fresh regen.
```

---

## Final Validation Checklist

### Technical Validation
- [ ] `bash docs/generate_llms_full.sh` → "wrote …/docs/llms_full.txt (… lines, … bytes)"
- [ ] `test -s docs/llms_full.txt` → bundle non-empty
- [ ] `grep -c '^ \([0-9]\+\.\) ' docs/llms_full.txt` → 8 section banners
- [ ] Determinism: regen twice → `diff` shows no change
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` → 0 failed (exit 0)
- [ ] `cargo clippy --all-targets -- -D warnings` → exit 0, no warnings
- [ ] Host-OS clippy caveat documented (Linux preferred for F16 backend coverage)

### Feature Validation
- [ ] `docs/llms_full.txt` stale-claim grep (`hyprland only`/`not supported yet`) → 0 hits
- [ ] `docs/llms_full.txt` cross-DE grep (`foreign-toplevel`/`backend selection`/`atspi`/…) → ≥1 hit
- [ ] §5→backend traceability table populated; all 8 DEs have a backing backend
- [ ] `git status --porcelain` changes limited to `docs/llms_full.txt` (+ optional `src/` clippy fix)

### Code Quality / Docs
- [ ] No hand-edits to `docs/llms_full.txt` (generated artifact only)
- [ ] Any `src/` clippy fix is behavior-preserving (lint-only, no logic change)
- [ ] `spec/PRD.md`, `spec/*.md`, `tasks.json`, `prd_snapshot.md`, `.gitignore`,
      `packaging/*`, `.github/workflows/*`, `Cargo.toml`, `docs/*.md` all UNTOUCHED

### Documentation & Deployment
- [ ] `docs/llms_full.txt` reflects the current README + docs/*.md (no stale text)
- [ ] The §5→backend mapping recorded as this task's traceability output
- [ ] Any deviation from the expected host-OS (non-Linux clippy) is noted

---

## Anti-Patterns to Avoid

- ❌ Don't hand-edit `docs/llms_full.txt` — it is a *generated* artifact. Re-run
     `generate_llms_full.sh` if it's wrong.
- ❌ Don't drop `--test-threads=1` from the test command — the notifier/debouncer
     shares global mutable state; parallel tests race and give false failures.
- ❌ Don't drop `-D warnings` from clippy, and don't add `--no-deps` thinking it
     silences project lints — `-D warnings` on `--all-targets` is the actual gate.
- ❌ Don't treat a CLEAN `git diff docs/llms_full.txt` as a failure — it means a
     prior regen already captured the edits. Clean is acceptable.
- ❌ Don't run the regen BEFORE P2.M7.T1.S1's markdown edits land and then call it
     done — verify the stale strings are gone from the *inputs* first (Task 0).
     If they're not, note the dependency; the bundle will need one more regen.
- ❌ Don't edit `spec/PRD.md` or any `spec/*.md` to "fix" the §5 matrix — those are
     human-owned, read-only. If a claim is unbacked, *flag* it; don't rewrite it.
     (Today all claims are backed, so this is confirm-only.)
- ❌ Don't expand scope into `packaging/*`, `.github/workflows/*` (owned by the
     parallel P2.M7.T2.S1), `Cargo.toml`, `flake.nix`, or `docs/*.md` (owned by
     P2.M7.T1.S1). Your diff is `docs/llms_full.txt` ± behavior-preserving clippy
     fixes in `src/`.
- ❌ Don't skip clippy "because tests pass" — `-D warnings` is a separate gate and
     the explicit milestone requirement. Fix flagged lints; don't suppress them
     globally.

---

## Confidence Score

**9/10** — One-pass success is highly likely because every step is a concrete,
verified command with a known-good expectation: the generator is idempotent and
OS-independent; the exact test command (`--test-threads=1`) and exact clippy gate
(`--all-targets -- -D warnings`) are taken verbatim from AGENTS.md and the
verified project config; the §5→backend mapping is pre-verified against
`linux_backend_candidates()`/`construct_backend()` and the `[features]` table
(all 8 DEs backed). The one residual risk (−1) is that the F16 backend source
(P2.M2-P2.M5) may carry a clippy lint this task must fix behavior-preservingly on
a Linux host — which is straightforward but host-dependent, and CI backstops it.