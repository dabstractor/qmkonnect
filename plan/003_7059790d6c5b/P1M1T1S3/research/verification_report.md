# Verification Report — P1.M1.T1.S3: Full-tree verification grep and cargo check

- **Task ID:** P1.M1.T1.S3
- **Task type:** Mode C verification gate (closes P1.M1.T1)
- **Date (UTC):** 2026-07-20T10:06:16Z
- **Repo:** `/home/dustin/projects/qmkonnect`
- **Command CWD:** `/home/dustin/projects/qmkonnect`
- **Scope:** Read-only verification pass of the v0.2.4→v0.2.8 naming-drift
  remediation (S1 source fix + S2 generated-mirror regen). Confirms the entire
  *product* tree is clean of stale tokens AND the crate compiles cleanly.
  No product-tree file was modified (all 6 checks passed).

---

## Summary

**All 6/6 checks PASS.** The v0.2.8 naming-drift remediation is internally
consistent across the shipped product tree. Every contract grep returns zero
product-tree hits, and `cargo check --bin qmkonnect --offline` exits 0 with no
warnings. P1.M1.T1 ("Fix residual doc drift and verify clean tree") verification
gate **PASSES**.

| Result        | Count |
| ------------- | ----- |
| PASS          | 6     |
| FAIL          | 0     |
| Known obs.    | 2     (non-failures, documented below) |
| Product-tree files modified | 0 |

---

## Inputs confirmed (Task 1 pre-flight)

Before verifying the whole tree, the upstream siblings' deliverables were
confirmed present and clean:

| Check | Command | Expected | Observed | Result |
| ----- | ------- | -------- | -------- | ------ |
| S1 landed | `grep -n 'qmk_notifier_notify' docs/troubleshooting.md` | one hit, line 647 | `647:   (there is no built-in \`qmk_notifier_notify\` callback — the firmware API is the` | ✅ PASS |
| S2 landed | `grep -n 'qmk_notifier_notify' docs/llms_full.txt` | one hit, ≈ line 2622 | `2622:   (there is no built-in \`qmk_notifier_notify\` callback — the firmware API is the` | ✅ PASS |
| Both clean of stale form | `grep -n 'qmk-notifier_notify' docs/troubleshooting.md docs/llms_full.txt` | no output (exit 1) | no output, exit 1 | ✅ PASS |

The fix-or-fail branch did **not** trigger: both inputs are clean.

---

## Per-check results (the 6 checks)

Exclusion filter reused for repo-wide greps (a, b, c, e):
```
EXCL='\.pi-subagents/|/target/|docs/vendor/|/\.git/|/plan/'
```
For repo-wide greps, both the **raw** count (everything the pattern matched) and
the **filtered** count (product tree only) are recorded, so the exclusions are
auditable and a product-tree hit could not be quietly dropped.

| # | Command | Scope / exclusions | Expected | Observed | Exit | Result |
| - | ------- | ------------------ | -------- | -------- | ---- | ------ |
| (a) | `grep -rn 'qmk-notifier_notify' . \| grep -vE "$EXCL"` | repo-wide, `$EXCL` filter | no output | raw **116** hits (all under `plan/` + `.pi-subagents/`); **filtered: 0** product-tree hits | 1 (last grep) | ✅ PASS |
| (b) | `grep -rn 'package = "qmk_notifier"' --include='*.toml' . \| grep -vE "$EXCL"` | repo-wide, `*.toml`, `$EXCL` filter | no output | raw **0** hits; **filtered: 0** | 1 | ✅ PASS |
| (c) | `grep -rn 'tag = "v0.2.1"' --include='*.toml' . \| grep -vE "$EXCL"` | repo-wide, `*.toml`, `$EXCL` filter | no output | raw **0** hits; **filtered: 0** | 1 | ✅ PASS |
| (d) | `grep -rn 'build-installer.ps1' .github/` | `.github/` only (NOT widened) | no output | no output | 1 | ✅ PASS |
| (e) | `grep -rn 'config/qmk-notifier' --include='*.rs' --include='*.md' . \| grep -vE "$EXCL"` | repo-wide, `*.rs`/`*.md`, `$EXCL` filter | no output | raw **47** hits (all under excluded dirs); **filtered: 0** product-tree hits | 1 | ✅ PASS |
| (f) | `cargo check --bin qmkonnect --offline` | full build gate | exit 0, "Finished \`dev\` profile …", **no warnings** | `Finished \`dev\` profile [unoptimized + debuginfo] target(s) in 0.14s`, **zero warnings** | 0 | ✅ PASS |

### Exit-code semantics

For checks (a)–(e), `grep` exit **1 = no match = PASS**; exit **0 = match found =
FAILURE**. The pipeline exit code reported is the last `grep`'s (`grep -vE`).
For check (f), cargo exit **0 = PASS**. All verdicts are additionally backed by
the capture-then-filter auditable form (raw file inspected + filtered line count
explicitly counted), so a hit that the exclusion accidentally kept could not be
masked.

### Cargo determinism

A re-run of check (f) reproduced the result:
```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.08s
```
The `--offline` flag worked as specified — the `qmk-notifier` v0.3.0 git dep is
cached locally (Cargo.lock pins it). **No `--offline` deviation** occurred.

---

## Known observations (NON-failures — documented for completeness)

### Observation (i): `installer.wxs` is ABSENT

- The item description and the S2 PRP context carry a clause to exclude "the
  explicitly-retained `packaging/windows/installer.wxs` legacy file." That clause
  is **stale**: commit **`cb9a165` "ci(windows): remove legacy WiX tooling"**
  deleted `installer.wxs` (and `build-installer.ps1`) from the product tree.
- Confirmed: `find . -name '*.wxs' 2>/dev/null | grep -vE "$EXCL"` → **empty**.
- Confirmed: `git log --oneline -1 -- '**/installer.wxs'` →
  `cb9a165 ci(windows): remove legacy WiX tooling`.
- Excluding a nonexistent path is a harmless no-op. **No action taken** — the file
  is correctly absent and is not restored.

### Observation (ii): `spec/PACKAGING.md:88,232` reference removed WiX tooling

- `spec/PACKAGING.md` still describes the *removed* legacy WiX tooling:
  - **Line 88:** `` `packaging/windows/installer.wxs` + `build-installer.ps1` (needs WiX v3) build ``
  - **Line 232:** `` (`build-installer.ps1`) is not invoked by CI. ``
  - Confirmed: `grep -n 'build-installer.ps1' spec/PACKAGING.md` → lines **88** and **232**.
- These ARE stale doc references, BUT they are **out of scope** of contract grep
  (d), which is scoped to `.github/` ONLY. A repo-wide grep would hit them; the
  contract scope deliberately excludes them. The actual check (d)
  (`grep -rn 'build-installer.ps1' .github/`) returns **no output** (exit 1), so
  this observation does **not** fail the gate.
- **Not actioned here.** This is a separate doc-drift work item (the S3 contract
  is "DOCS: none — verification pass"). Recording it here so it is not lost.

### Non-observation: legitimate labels in `spec/HOST_RULES.md`

For the record (NOT a discrepancy — these are correct and were left untouched):
- **Line 563:** `qmk-notifier/  (external crate)` — hyphen = Rust crate repo (correct)
- **Line 565:** `qmk_notifier/  (external firmware)` — underscore = firmware repo (correct)

None of the 5 contract greps match these labels (grep (b) needs `package =`,
grep (e) needs a `config/` prefix). They follow the convention exactly.

---

## Conclusion

**P1.M1.T1 verification gate PASSES.**

- All 5 contract greps return **zero** product-tree hits (exit 1 / filtered empty).
- `cargo check --bin qmkonnect --offline` exits **0** with **zero warnings**.
- Both upstream deliverables (S1: `docs/troubleshooting.md:647`, S2:
  `docs/llms_full.txt:2622`) are present and read `qmk_notifier_notify`
  (underscore form); neither file contains the stale hyphen form.
- The two known observations are non-failures: (i) `installer.wxs` is correctly
  absent (removed `cb9a165`; stale exclusion clause is a no-op), and (ii)
  `spec/PACKAGING.md:88,232` legacy WiX refs are out of contract grep (d)'s
  `.github/`-only scope and are recorded as a separate work item.
- **No product-tree file was modified.** The fix-or-fail branch did not trigger.
  The only artifact produced by this task is this report.

P1.M1.T1 ("Fix residual doc drift and verify clean tree") is complete; the
mileeline M1 verification gate is closed with auditable evidence. A future agent
can re-run the exact 6 commands above (with the exact `$EXCL` filter and the
exact `.github/` scope for grep (d)) and reproduce this verdict.