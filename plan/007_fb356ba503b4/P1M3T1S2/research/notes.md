# Research Notes — P1.M3.T1.S2 (Scoop bucket repo + update-manifest.ps1)

Source: 2 subagent research calls (Scoop bucket/validation/pwsh + Homebrew tap structural
template) + direct reads of the codebase. Two primary in-source precedents were read in full:
`packaging/homebrew/tap-README.md` and `packaging/homebrew/update-cask.sh` (the exact Scoop
analogues, both COMPLETE).

## 0. Naming truth (GROUND TRUTH — resolved against `git remote`)

- `git remote get-url origin` → `git@github.com:dabstractor/qmkonnect.git`
  ⇒ **GitHub org = `dabstractor`** (the local Linux user `dustin` in `/home/dustin/...` is
  unrelated and must NOT be confused with the org).
- Source repo = `dabstractor/qmkonnect`.
- Scoop bucket repo = **`dabstractor/scoop-qmkonnect`** (correct spelling, WITH the `c`).
  Confirmed by: `architecture/external_deps.md` §3, `tasks.json` (both the P1.M3.T1.S2 and the
  downstream P1.M5.T1.S2 CI contracts), and the Homebrew precedent `dabstractor/homebrew-qmkonnect`.
- **S1 shipped the typo `scoop-qmkonnet`** (missing `c`) throughout
  `packaging/scoop/README.md` (it does NOT appear in `qmkonnect.json`, whose only org ref is the
  source `homepage`/`url`). S2 must use the correct `scoop-qmkonnect` and apply a surgical
  consistency-fix to S1's README (`scoop-qmkonnet` → `scoop-qmkonnect`).

## 1. Scoop bucket repo structure (convention)

- A Scoop **bucket** is a git repo named `scoop-<name>` containing a **`bucket/`** directory that
  holds the app manifests as `bucket/<app>.json` (filename = app name = `qmkonnect.json`), plus a
  root `README.md`. This is the official `ScoopInstaller/BucketTemplate` layout.
- It mirrors a Homebrew tap 1:1: tap = `<name>-<tapname>` repo + `Casks/<app>.rb` + root README;
  bucket = `scoop-<name>` repo + `bucket/<app>.json` + root README.
- ⇒ `packaging/scoop/bucket-README.md` (this task) is the README that lives at the ROOT of
  `dabstractor/scoop-qmkonnect`. CI (P1.M5.T1.S2) copies `packaging/scoop/qmkonnect.json` into the
  bucket clone as `bucket/qmkonnect.json`.

## 2. CRITICAL — manifest validation (the contract's `scoop checkup` is WRONG)

- `scoop checkup` inspects the **Scoop installation / environment health** (admin status, Defender
  exclusions, helper tools, long-path support, network) — it NEVER inspects a manifest. The task
  contract says "optionally validates with `scoop checkup`"; that command does not do what the
  contract implies.
- The real per-manifest validators are:
  1. `scoop install <manifest-or-app>` — the strongest check (smoke install).
  2. `scoop checkver <app>` (or the bucket's `bin/checkver.ps1`) — runs `checkver` + exercises the
     `autoupdate` template (downloads, computes hash).
  3. `bin/checkurls.ps1` — URL liveness (HTTP 200).
  4. JSON-schema validation against `ScoopInstaller/Scoop/schema.json`.
- DECISION for `update-manifest.ps1`: the ALWAYS-ON check is a JSON well-formedness re-parse
  (`Get-Content -Raw | ConvertFrom-Json` parse-only — does NOT rewrite the file) +
  `Select-String` confirmation that the 3 patched values landed. The OPTIONAL, Windows-only check
  is `scoop checkver` (NOT `scoop checkup`) when `scoop` is on PATH. Document this honestly in the
  script header + bucket-README.

## 3. Autoupdate vs manual/script patching

- A manifest with `autoupdate.architecture.64bit.url` (carrying `$version`) and NO `hash` is
  refreshed by `scoop checkver -u` (there is no standalone `scoop autoupdate` command) — it
  downloads the asset and computes SHA256 itself.
- **You CAN patch `version` + `architecture.64bit.url` + `architecture.64bit.hash` directly by
  hand/script and publish without Scoop at all** — the top-level fields are what's read at install
  time; the `autoupdate` block is only consumed by Scoop's own update tooling. CAVEAT: keep the
  `autoupdate` `$version` URL template CONSISTENT with the patched top-level URL (same pattern),
  or `checkver` will flag a spurious mismatch. Our manifest satisfies this (S1's autoupdate url is
  exactly the top-level url with `$version` substituted), so patching version+url+hash is safe.
- ⇒ `update-manifest.ps1` patches exactly: top-level `version`, `architecture.64bit.url`
  (concrete), `architecture.64bit.hash`. It does NOT touch `checkver` or `autoupdate`.

## 4. CI publish pattern (deploy key)

- Clone bucket via SSH **deploy key** (write-enabled) → patch → commit → push is the conventional
  approach. Use `webfactory/agents/github-ssh-agent@v0.9.0` to load the key.
- No single "official" Scoop Action covers the push-on-release model: `ScoopInstaller/Excavator`
  is the inverse (scheduled PULL of upstream changes), `ScoopInstaller/GithubAction` just INSTALLS
  Scoop in CI. So we use plain `git` + the deploy key.
- Secret name `SCOOP_BUCKET_DEPLOY_KEY` is a sensible community convention (mirrors
  `HOMEBREW_TAP_DEPLOY_KEY`); the downstream CI task P1.M5.T1.S2 wires it. A PAT (repo-scope) is
  an HTTPS alternative, but the deploy key is preferred (scoped, revocable).

## 5. PowerShell specifics (cross-platform: Windows pwsh 5.1 + GitHub Actions ubuntu/windows pwsh 7)

- **Download:** `Invoke-WebRequest -Uri $url -OutFile $tmp`. It follows GitHub's 302 release
  redirect transparently. Add `-UseBasicParsing` for Windows PowerShell 5.1 compatibility (harmless
  on 7). `pwsh` 7 ignores it.
- **SHA256:** `$hash = (Get-FileHash -Algorithm SHA256 $tmp).Hash.ToLower()` → 64 lowercase hex.
- **AVOID `ConvertFrom-Json`/`ConvertTo-Json` to REWRITE the manifest:** PS 5.1 truncates at depth
  2, and both reorder keys / reflow whitespace / drift the `"##"` comment key out of its leading
  position. Use targeted regex line-patching instead (the Scoop equivalent of the Homebrew
  script's `sed` line-patch).
- **`$`-substitution trap in `-replace`:** the RHS of `-replace` is a substitution string where
  `$<name>` / `$1` reference capture groups. `$Version` in the RHS would be read as a named-group
  reference and silently drop. FIX: use `[regex]::Replace($content, $pattern, { param($m) … })`
  with a **MatchEvaluator scriptblock** — inside the scriptblock, `$Version`/`$Sha256` are ordinary
  PowerShell variables (closure), so no substitution ambiguity. This is the correct, readable form.
- **Targeting the concrete `url` vs the `$version` template:** there are two `"url"` keys in the
  manifest (the concrete `architecture.64bit.url` and the `autoupdate.architecture.64bit.url`
  template). A MatchEvaluator that checks `if ($value -notmatch '\$version')` leaves the template
  untouched and rewrites only the concrete one. (See PRP "Implementation Patterns".)

## 6. Homebrew tap structural template (the precedent to mirror)

- `tap-README.md` = 8 sections: *What this is* → *Install* → *What it installs* (table) →
  *Uninstall* → *For maintainers — updating the cask* → *CI publishing (deploy key)* → *Path to the
  official cask* → *See also*. Tone: authoritative, em-dash-heavy, parenthetical PRD/spec citations,
  inline `#`-comments inside command blocks explaining non-obvious flags.
- `update-cask.sh` signature/guards: args `VERSION` then optional `SHA256` (+ `--help`);
  `set -euo pipefail`; **leading-`v` guard**; SHA256 format validation (len 64 + `^[0-9a-f]{64}$`);
  BSD/GNU-sed portable `sed -i.bak -E` + `rm -f .bak`; mandatory **post-patch `grep -q` confirm**;
  best-effort `brew audit` (skips if no `brew`). Explicit scope clause:
  *"This script is a PURE local file update — it does NOT push…"*. Portability note:
  macOS ships `shasum`/BSD sed; Linux ships `sha256sum`/GNU sed.
- `bucket-README.md` (this task) reproduces the tap-README skeleton; `update-manifest.ps1`
  reproduces update-cask.sh's flow in PowerShell (download → Get-FileHash → regex-patch → confirm →
  optional checkver), with the same scope clause and the same deploy-key doc.

## 7. Input manifest facts (S1's `packaging/scoop/qmkonnect.json`, read in full)

The script patches these fields (paths):
- `"version": "0.2.8"` — top-level scalar.
- `architecture.64bit.url` — concrete, literal `…/v0.2.8/QMKonnect-0.2.8-windows-x64.exe`.
- `architecture.64bit.hash` — 64-zero placeholder (CI-fill).
- `autoupdate.architecture.64bit.url` — TEMPLATE (`$version`); **must NOT be patched**; stays as-is.
- `checkver` — `{github, regex}`; static; **must NOT be patched**.

Release-asset facts (verified in `.github/workflows/release.yml` + `QMKonnect.iss`):
- Asset = `QMKonnect-<version>-windows-x64.exe` (Inno `QMKonnect-Setup.exe` renamed in CI;
  `Move-Item Output/QMKonnect-Setup.exe "Output/QMKonnect-<ver>-windows-x64.exe"`).
- Version is bare, NO leading `v` (`cargo metadata … | ConvertFrom-Json`). The URL path adds the
  `v` (`…/v<ver>/…`); the asset filename uses the bare `<ver>`.
- NO `.sha256` sidecar is published (`grep sha256|sidecar release.yml` → none) ⇒ the script MUST
  compute the hash itself from the download (no sidecar to read).
- DestName = `QMKonnect.exe` (Inno `MyAppExeName`); `PrivilegesRequired=lowest` (per-user);
  `ArchitecturesAllowed=x64compatible` (x64-only); HKCU `Run` autostart default-on; AppId GUID
  stable; `[Code] CurStepChanged` runs `set_aumid.ps1` → AUMID `Mulletware.QMKonnect` (APP_AUMID in
  `src/platforms/mod.rs`).