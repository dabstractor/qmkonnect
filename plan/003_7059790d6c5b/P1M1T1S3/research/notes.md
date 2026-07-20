# Research Notes — P1.M1.T1.S3: Full-tree verification grep and cargo check

## Task nature

This is a **VERIFICATION / CONFIRMATION** task, not a remediation task. Its
deliverable is a *verification report*, not a code/doc change. It runs 5
repo-wide greps + `cargo check --offline` to confirm the v0.2.4→v0.2.8 naming
drift remediation (S1 source fix + S2 generated-mirror regen) has cleared the
entire *product* tree.

## Baseline state (verified directly in the working tree, 2025-07-20)

### S1 + S2 are BOTH LANDED in the working tree

- `docs/troubleshooting.md:647` reads `qmk_notifier_notify` (underscore) — S1 done.
- `docs/llms_full.txt:2622` reads `qmk_notifier_notify` (underscore) — S2 done
  (`git status` shows `docs/llms_full.txt` modified).
- `git status --short` shows: ` M docs/llms_full.txt`, ` M plan/.../tasks.json`,
  `?? plan/.../P1M1T1S2/`. NOTE: `docs/troubleshooting.md` is NOT in working-tree
  diff → S1 was COMMITTED (not just edited). Either way, the content is fixed.

### All 5 contract greps ALREADY PASS (exit 1 = no match)

Run with exclusions `grep -vE '\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'`:

| # | Grep | Scope | Expected | Baseline result |
|---|------|-------|----------|-----------------|
| a | `qmk-notifier_notify` | whole repo | ZERO | ✅ ZERO |
| b | `package = "qmk_notifier"` | `--include='*.toml'` | ZERO | ✅ ZERO |
| c | `tag = "v0.2.1"` | `--include='*.toml'` | ZERO | ✅ ZERO |
| d | `build-installer.ps1` | `.github/` ONLY | ZERO | ✅ ZERO |
| e | `config/qmk-notifier` | `--include='*.rs' --include='*.md'` | ZERO | ✅ ZERO |

### `cargo check --bin qmkonnect --offline` PASSES

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.13s
```
Exit 0, NO warnings. The `qmk-notifier` v0.3.0 git dep IS cached in the local
cargo registry, so `--offline` succeeds. Cargo.lock confirms:
`qmk-notifier 0.3.0 (git+...?tag=v0.3.0#f26893e...)`.

## CRITICAL discrepancy #1 — `installer.wxs` DOES NOT EXIST

The item description and the S2 PRP context both say:
> "re-running these greps excluding `.pi-subagents/` artifacts and the
>  explicitly-retained `packaging/windows/installer.wxs` legacy file."

**But there is NO `installer.wxs` anywhere in the repo.** Evidence:
- `find . -name '*.wxs'` → no results (exit 0).
- `git log --oneline --all -- '**/installer.wxs'` →
  `cb9a165 ci(windows): remove legacy WiX tooling`.

So the WiX tooling (including `installer.wxs` and `build-installer.ps1`) was
**removed** in commit `cb9a165`. The "explicitly-retained" clause is STALE.

**Implication for S3:** the exclusion of `installer.wxs` is a no-op (you cannot
exclude a file that doesn't exist). The implementing agent must NOT:
- fail because `installer.wxs` is "missing", nor
- try to "restore" or create it.

The contract greps do not reference `installer.wxs` directly (only `build-installer.ps1`
in `.github/`, which is correctly scoped and returns zero).

## CRITICAL observation #2 — `spec/PACKAGING.md` has legacy WiX refs OUT of scope

`spec/PACKAGING.md` contains:
- `:88`:  `` `packaging/windows/installer.wxs` + `build-installer.ps1` (needs WiX v3) build ...``
- `:232`: `(`build-installer.ps1`) is not invoked by CI.`

These describe the **removed** legacy WiX tooling — i.e. they are stale doc refs.

**BUT:** contract grep (d) is scoped to `.github/` ONLY. `spec/PACKAGING.md` is
NOT in `.github/`, so it is **OUT OF SCOPE** of the contract verification. A
repo-wide `grep -rn 'build-installer.ps1' .` WOULD hit spec/PACKAGING.md:88,232 —
which is exactly why grep (d) is `.github/`-scoped in the contract.

**Implication for S3:** the agent must run grep (d) with the EXACT `.github/`
scope from the contract. It should NOTE spec/PACKAGING.md's legacy refs in the
verification report as a known/accepted out-of-contract observation (legacy doc
describing removed tooling), NOT treat them as a contract failure, and NOT fix
them in this task (fixing spec/ doc drift is a separate work item, out of scope).

## Why the exclusions are MANDATORY (not optional)

A repo-wide `grep -rn 'qmk-notifier_notify' .` (no exclusions) returns ~30 hits,
ALL legitimate — they live in the PLANNING record, not the product tree:
- `plan/003_.../architecture/delta_verification.md` — the drift report (documents
  the stale token it found).
- `plan/003_.../delta_prd.md` — the delta PRD (names the token to fix).
- `plan/003_.../P1M1T1S1/PRP.md`, `P1M1T1S2/PRP.md`, `research/notes.md` — the
  PRPs that describe the before/after fix (legitimately quote the old form).
- `plan/003_.../tasks.json` — task descriptions.
- `.pi-subagents/artifacts/...` — cached subagent transcripts/outputs that
  reference the token during research.

None of these is "product drift." Excluding `.pi-subagents/`, `target/`,
`docs/vendor/`, `.git/`, `plan/` isolates the greps to the shipped product tree
(src/, docs/, spec/, Cargo.toml, .github/, packaging/, README, etc.).

## Legitimate tree labels that must NOT be "fixed"

`spec/HOST_RULES.md:563` has a file-tree diagram with:
```
qmk-notifier/  (external crate)     <- the Rust crate repo (hyphen = CORRECT)
qmk_notifier/  (external firmware)  <- the firmware C module (underscore = CORRECT)
```
These are LABELS for the two external repos, not config paths and not package
declarations. Contract grep (e) `config/qmk-notifier` does NOT match them (no
`config/` prefix), and grep (b) `package = "qmk_notifier"` does not match (no
`package = `). They are correct as-is and must be left untouched.

## Where the deliverable report goes

The contract says "DOCS: none — this is a verification pass, no doc surface
change." So the verification report must NOT go into the product docs tree
(`docs/` or `spec/`). It goes in the plan research area:
`plan/003_7059790d6c5b/P1M1T1S3/research/verification_report.md`.

## Validation approach

There are no unit tests for a grep/cargo-check verification pass. The
"validation" is:
1. Each of the 5 greps exits non-zero (no matches) within the product tree.
2. `cargo check --bin qmkonnect --offline` exits 0 with no warnings.
3. The verification report documents all 6 checks + the two known observations
   (installer.wxs absence, spec/PACKAGING.md out-of-scope refs).
4. If ANY grep unexpectedly hits (a real product-tree stale ref), the agent
   must FIX it before declaring the task complete (per contract OUTPUT clause).

## Sources verified
- `plan/003_7059790d6c5b/architecture/delta_verification.md` — §Clean Grep Results.
- `Cargo.toml:18` — `qmk-notifier = { git=..., tag = "v0.3.0" }` (correct).
- `Cargo.lock` — `qmk-notifier 0.3.0` git source (correct).
- `git log cb9a165` — WiX tooling removal (explains installer.wxs absence).
- `spec/PACKAGING.md:88,232` — legacy WiX refs (out of grep-d scope).
- `spec/HOST_RULES.md:555-570` — legitimate qmk-notifier/ + qmk_notifier/ labels.