# Research Notes — P1.M5.T2.S1: Add Winget publishing CI job

## Task
Append ONE `winget` job to `.github/workflows/release.yml` that opens a per-release PR to
`microsoft/winget-pkgs` for `dabstractor.QMKonnect` (the Windows community channel, PRD §4 F15).
The contract specifies the `vedantmgoyal9/winget-releaser` action as the primary mechanism.
Single-file change; the `WINGET_GITHUB_TOKEN` secret is documented inline (Mode A ride-along).

## Ground truth (verified on this box, this session)
- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git` ⇒ **org = `dabstractor`**.
  (Local Linux user `dustin` is UNRELATED to the org.)
- **Cargo version = `0.2.8`.** Release tags are `v0.2.8`. Winget `PackageVersion` + asset name are BARE.
- Asset = `QMKonnect-<version>-windows-x64.exe` (the `windows` job renames `QMKonnect-Setup.exe` → this).
- `WINGET_GITHUB_TOKEN` is NOT referenced anywhere in `.github/` yet (grep → none). I am adding it.
- **No existing `winget` job** in release.yml (grep `^\s{2}winget:` → none). Good.
- **Current release.yml job inventory (in order):** `macos`, `windows`, `linux-binary`, `arch`,
  `publish`, `aur`, `homebrew-tap`, `scoop-bucket` (538 lines total). P1.M5.T1.S1 (aur) = Complete;
  P1.M5.T1.S2 (homebrew-tap + scoop-bucket) = **has LANDED** in the file (last job is `scoop-bucket`).
  ⇒ **Append the `winget` job at the very END, after `scoop-bucket`.**
- `pwsh` 7.6.2 IS on this box (not needed for THIS task — the job runs ubuntu-latest bash for the
  `steps.ver` shell step; the winget-releaser action itself uses pwsh internally via the composite action).

## The load-bearing external dep: `vedantmgoyal9/winget-releaser@v2` — action.yml read in FULL

Source of truth: <https://github.com/vedantmgoyal9/winget-releaser/blob/master/action.yml> (read verbatim
this session). It is a **composite action** (not Docker, not node) that shells out to **Komac**
(`cargo binstall komac`), NOT wingetcreate. Cross-platform → runs on **ubuntu-latest**.

### Exact `inputs:` (from action.yml)
| input | required | default | our value |
|---|---|---|---|
| `identifier` | **true** | — | `dabstractor.QMKonnect` |
| `version` | false | (derived — see below) | `${{ steps.ver.outputs.version }}` (BARE 0.2.8) |
| `installers-regex` | true | `.(exe\|msi\|msix\|appx)(bundle){0,1}$` | `QMKonnect-.*-windows-x64\.exe$` |
| `max-versions-to-keep` | true | `'0'` (keep all) | OMIT (default is fine) |
| `release-repository` | true | `${{ github.event.repository.name }}` | **OMIT** (defaults to `qmkonnect`) |
| `release-tag` | true | `${{ github.event.release.tag_name \|\| github.ref_name }}` | **OMIT** |
| `release-notes-url` | false | — | OMIT |
| `token` | **true** | — | `${{ secrets.WINGET_GITHUB_TOKEN }}` |
| `fork-user` | true | `${{ github.repository_owner }}` | OMIT (defaults to `dabstractor`) |

### How the action resolves the release on a PUSH event (the key correctness fact)
The "get release information" step calls:
```
Invoke-RestMethod -Uri 'https://api.github.com/repos/${{ github.repository_owner }}/${{ inputs.release-repository }}/releases/tags/${{ inputs.release-tag }}'
```
- **`release-tag` default = `${{ github.event.release.tag_name || github.ref_name }}`.** On our `push`-triggered
  workflow, `github.event.release` is **null** (push event, not release event) → falls back to
  `github.ref_name` = the tag (`v0.2.8`). **So the action works on push events with NO explicit `release-tag`.** ✓
- **`release-repository` default = `${{ github.event.repository.name }}`** = `qmkonnect` (NAME ONLY).
  The action **prepends `${{ github.repository_owner }}/`** = `dabstractor/` → final repo path `dabstractor/qmkonnect`. ✓

  ⚠️ **CRITICAL CORRECTION:** the README snippet in `packaging/winget/README.md` (authored by P1.M3.T2.S2
  from a *description*, not the raw action.yml) shows `release-repository: dabstractor/qmkonnect`. That would
  produce `dabstractor/dabstractor/qmkonnect` → **404**. **OMIT `release-repository`** (default `qmkonnect` is
  correct), OR set it to the bare name `qmkonnect`. The contract does NOT list `release-repository`, so OMIT it.

### `version` handling (the version gotcha — confirmed VERBATIM from action.yml)
```
If ('' -eq '${{ inputs.version }}') {
  Write-Output "version=$($ReleaseInfo.tag_name -replace '^v')" >> $env:GITHUB_OUTPUT   # strip leading v
} Else {
  Write-Output "version=${{ inputs.version }}" >> $env:GITHUB_OUTPUT                     # VERBATIM, no strip
}
```
- OMIT `version` → action strips `^v` from the tag → bare `0.2.8`. ✓
- PROVIDE `version` → used **verbatim** (NOT stripped).

  The contract says `version: ${{ steps.ver.outputs.version }}`. To honor it AND avoid the gotcha,
  `steps.ver` must yield the **bare** version. We compute it with the SAME `${GITHUB_REF_NAME#v}` idiom
  the sibling `homebrew-tap` / `scoop-bucket` jobs use → `0.2.8`. Passing that bare value verbatim is correct.
  **NEVER** pass `${{ github.event.release.tag_name }}` (null on push) or the raw `github.ref_name` (`v0.2.8`).

### What the action does internally (so we understand its failure modes + prereqs)
1. **Pre-flight:** `Invoke-WebRequest ... winget-pkgs/tree/master/manifests/d/da/dabstractor/QMKonnect -Method Head`;
   if the package dir does NOT exist → `::error::Package dabstractor.QMKonnect does not exist in the
   winget-pkgs repository. Please add atleast one version...` + `exit 1`. ⇒ **Confirms the one-time manual
   `wingetcreate new` prerequisite** (documented in `packaging/winget/README.md`). The job WILL fail on the
   first release until that initial PR is merged — expected, not a bug.
2. `cargo-bins/cargo-binstall@main` (⚠️ the action pins its OWN binstall dep to `@main` — a moving target;
   supply-chain note, but it is the action's internal choice, NOT something we control) + `cargo binstall komac -y`.
3. Fetch the release by tag (uses `github.token` to read OUR release — fine under top-level `contents: read`).
4. Resolve `version` (above) + `urls` = the release assets whose name `-match` the installers-regex.
5. `komac sync-fork` (sync `<fork-user>/winget-pkgs` = `dabstractor/winget-pkgs`; uses the PAT `inputs.token`).
6. `komac update 'dabstractor.QMKonnect' --version '<bare>' --submit --urls <asset-url>` (Komac computes the
   SHA256 + opens the PR `fork → microsoft:main`; uses the PAT).
7. `komac cleanup --only-merged`.

**Auth split:** the default `github.token` reads our release; the PAT (`inputs.token` = `WINGET_GITHUB_TOKEN`)
is used ONLY for Komac's fork/push/PR. So the PAT needs only `public_repo` scope. ⇒ **NO `permissions:`
escalation needed** on our job (top-level `contents: read` suffices; the PAT is not the GITHUB_TOKEN).

## CRITICAL CORRECTIONS to the work-item contract (apply ALL — each would fail CI)
1. **`@v2`, NOT `@latest`.** `git ls-remote --tags https://github.com/vedantmgoyal9/winget-releaser.git`
   shows `refs/tags/v2` (commit `4ffc7888`). There is **NO** `latest` ref → `vedantmgoyal9/winget-releaser@latest`
   would fail with *"Unable to resolve action … @latest, unable to find version"*. Pin to **`@v2`** (major).
   (GitHub Actions best practice is to pin a major anyway for reproducibility/supply-chain safety.)
2. **`installers-regex` typo.** The contract writes `QMKonnet-.*-windows-x64\.exe$`. The asset is
   `QMKonnect-…` (two c's). Use **`QMKonnect-.*-windows-x64\.exe$`** or the regex matches nothing → the
   action errors "no installer URLs found for the given regex".
3. **`version` must be BARE.** Pass `${{ steps.ver.outputs.version }}` where `steps.ver` strips the leading `v`
   (`echo "version=${GITHUB_REF_NAME#v}" >> "$GITHUB_OUTPUT"`). Never `${{ github.event.release.tag_name }}`
   (null on push → empty → action falls back to tag-strip anyway, but undefined behavior) and never the raw
   tag `v0.2.8` (verbatim → winget rejects).
4. **OMIT `release-repository`** (contract doesn't list it; the action owner-prepends). Setting it to
   `dabstractor/qmkonnect` (per the README snippet) DOUBLES the owner → 404. Default `qmkonnect` is correct.

## Why winget-releaser (Option B) and NOT submit.ps1 (Option A) for THIS job
The contract names `vedantmgoyal9/winget-releaser` as the **primary** mechanism with specific `with:` inputs,
and `submit.ps1` (windows-latest + wingetcreate) only as an *alternative*. This task implements the
contract's primary (Option B):
- **One action step** on `ubuntu-latest`; the action finds the .exe in the release, hashes it, opens the PR.
- **No checkout** needed (reads the release via API), no tool install in our workflow (Komac is installed
  inside the action).
- The `submit.ps1` path (Option A) is ALREADY fully implemented + tested in `packaging/winget/submit.ps1`
  (P1.M3.T2.S2, Complete) and documented as Option A in `packaging/winget/README.md`. If a future task wants
  to switch to it, the job would become: `runs-on: windows-latest`, `actions/checkout@v4`,
  `winget install Microsoft.WingetCreate`, `pwsh ./packaging/winget/submit.ps1 -Version <bare> -Submit
  -Token $env:WINGET_GITHUB_TOKEN`. **Out of scope here** — documented as the alternative for completeness.

## Coordination with the parallel P1.M5.T1.S2 (homebrew-tap + scoop-bucket)
- P1.M5.T1.S2 has **LANDED** (its `homebrew-tap` + `scoop-bucket` jobs are present; `scoop-bucket` is the
  last job at line ~483). ⇒ Append the `winget` job AFTER `scoop-bucket` (at the very END of the file).
- **Append-only, end-of-file** on both sides ⇒ the only conflict risk is a textual merge collision at the
  file's tail, which the orchestrator resolves. Functionally independent (each `needs: [publish]`).
- Mirror the sibling jobs' idioms VERBATIM: `needs: [publish]`, `if: github.event_name == 'push'`,
  `runs-on: ubuntu-latest`, the `# ─── banner` comment block + inline SECRET doc (Mode A), and the
  `${GITHUB_REF_NAME#v}` version step (consistency).

## Validation strategy (Linux dev box — no PAT, no winget-pkgs write)
- **YAML well-formedness:** `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/release.yml'))"`.
- **actionlint** (if installed): `actionlint .github/workflows/release.yml`.
- **grep invariants** (Validation Level 3): the `winget` job key; `needs: [publish]` + `if push`; the
  `vedantmgoyal9/winget-releaser@v2` reference (NOT @latest); the 4 contract inputs (identifier/version/
  installers-regex/token); the corrected regex (`QMKonnect`, not `QMKonnet`); `WINGET_GITHUB_TOKEN`
  referenced + documented; the `${GITHUB_REF_NAME#v}` bare-version step; NO `release-repository:
  dabstractor/...` (the owner-doubling bug); `git diff --stat` shows ONLY `.github/workflows/release.yml`.
- **DEFERRED to CI:** the real winget-pkgs PR on a tag push WITH the PAT set AND the one-time
  `wingetcreate new` merged. The dev box has neither → the live PR is a CI gate (honestly noted).

## Sources
- `vedantmgoyal9/winget-releaser` **action.yml** (read verbatim this session) — the exact inputs + the
  push-event `release-tag` fallback + the verbatim `version` handling + the pre-flight existence check.
- `git ls-remote --tags https://github.com/vedantmgoyal9/winget-releaser.git` → `refs/tags/v2` (confirms @v2).
- `packaging/winget/submit.ps1` (P1.M3.T2.S2, Complete) — the Option A alternative (NOT used by this job).
- `packaging/winget/README.md` "Publishing to microsoft/winget-pkgs" (P1.M3.T2.S2) — documents both options;
  its `release-repository: dabstractor/qmkonnect` snippet is the owner-doubling bug noted above.
- `plan/007_fb356ba503b4/P1M3T2S2/research/notes.md` — corroborating wingetcreate/winget-releaser findings.
- `plan/007_fb356ba503b4/architecture/external_deps.md` §4 (Winget) + "CI Publishing Strategy" (PR to
  winget-pkgs via wingetcreate or the official bot) + "Version Source of Truth" (Cargo.toml).
- In-repo ground truth: `.github/workflows/release.yml` (job inventory + `publish` + sibling jobs),
  `packaging/windows/inno/QMKonnet.iss` (Publisher/Name), `Cargo.toml` (version 0.2.8).