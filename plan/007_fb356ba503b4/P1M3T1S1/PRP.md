# PRP — P1.M3.T1.S1: Create Scoop app manifest for Inno installer

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging — no Rust/source/CI change.**
> **Two new files:** `packaging/scoop/qmkonnect.json` (the Scoop **app manifest**) +
> `packaging/scoop/README.md` (the source-repo packaging doc, **Mode A — rides with the work**).
> **Scope:** the manifest itself + its source-repo doc ONLY. The bucket-repo scaffolding + publish/autoupdate
> **script** is sibling **P1.M3.T1.S2** (the `dabstractor/scoop-qmkonnet` repo README + `update-manifest.sh`,
> mirroring Homebrew S2's `tap-README.md` + `update-cask.sh`); the CI push job is **P1.M5.T1.S2**.
> This task treats the manifest's `checkver`/`autoupdate` blocks as the CONTRACT that S2's automation consumes
> and never duplicates the publish script.
> **Parallel context:** P1.M2.T1.S2 (Homebrew tap, in-flight) owns `packaging/homebrew/*` — no file overlap; it
> is a STRUCTURAL/PATTERN precedent (bucket-repo + deploy-key CI model, version `v`-prefix rule).

---

## Goal

**Feature Goal**: Stand up the **Windows Scoop channel** for QMKonnect (PRD §4 F15; §5 — "Windows: Inno `.exe`
(primary, no admin) · Scoop · Winget"). A Scoop **app manifest** (`qmkonnect.json`) that downloads the per-tag
GitHub-release Inno installer `QMKonnect-<version>-windows-x64.exe`, extracts it (Scoop's `innosetup: true` runs
`innounp`, not the installer), and installs the self-contained tray app into the Scoop apps tree with a Start
Menu shortcut — plus a `checkver`/`autoupdate` block so `scoop checkup` detects new tags and fills
`version`/`url`/`hash` mechanically (no sidecar exists; hash is computed on update).

**Deliverable** (2 new files):
1. `packaging/scoop/qmkonnect.json` — a schema-valid Scoop app manifest: `version`, `description`, `homepage`,
   `license` (`MIT`), `innosetup: true`, `url` (the v0.2.8 release asset), `hash` (a 64-zero placeholder —
   schema-valid and install-blocking until CI fills the real SHA256), `shortcuts` (`[["QMKonnect","QMKonnect.exe"]]`),
   `checkver` (`{github, regex: "v([\\d.]+)"}`), `autoupdate` (`url` with `$version`; hash auto-computed).
2. `packaging/scoop/README.md` — the source-repo packaging doc: what the manifest is, the bucket+install commands
   (`scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet` + `scoop install qmkonnect`), what
   it installs, the **differences from the direct Inno installer** (the `innounp`-extraction delta), version & hash
   maintenance, and a maintainer section pointing to S2's bucket-repo + CI.

**Success Definition**:
- `jq . packaging/scoop/qmkonnect.json` → valid JSON, no errors.
- The manifest validates against the **authoritative Scoop schema** (`ScoopInstaller/Scoop` `schema.json`) — all
  required fields present (`version`, `homepage`, `license`, `url`), `hash` matches the 64-hex pattern,
  `shortcuts` is a valid 2-element array, `checkver` is the `{github, regex}` object form, `autoupdate.url`
  carries `$version`.
- `git diff --stat` shows ONLY the 2 new files under `packaging/scoop/`.
- The README contains the exact bucket+install commands, the autostart/AUMID deltas, and the S2/CI pointers.
- (Windows host, optional) `scoop install` of the manifest (after CI fills the real hash) extracts the exe and
  creates the Start Menu shortcut — deferred to a Windows box (Scoop is Windows-only; the dev box is Linux).

## User Persona (if applicable)

**Target User**: a Windows end-user who installs software via **Scoop** (per-user, no-admin, PATH-shimmed) and
wants QMKonnect managed/updated by `scoop update` alongside the direct Inno installer.

**Use Case**: `scoop bucket add qmkonnect <url>` → `scoop install qmkonnect` installs the tray app into the Scoop
tree + a Start Menu shortcut; `scoop update qmkonnect` pulls the next release (the manifest's `autoupdate` fills
the new version/url/hash from the next GitHub tag).

**User Journey**: (1) add the `dabstractor/scoop-qmkonnet` bucket; (2) `scoop install qmkonnect` → Scoop
downloads `QMKonnect-<ver>-windows-x64.exe`, extracts it via `innounp`, places `QMKonnect.exe`+icons in
`~\scoop\apps\qmkonnect\current\`, creates a Start Menu "QMKonnect" shortcut; (3) launch from Start Menu; (4)
enable "Open at Login" in the tray (the app writes its own HKCU Run value — see Differences); (5) `scoop update`
keeps it current.

**Pain Points Addressed**: gives Scoop users a native, `scoop update`-managed install path (F15) instead of
hand-downloading the `.exe`; per-user + no-admin matches Scoop's model and the Inno installer's
`PrivilegesRequired=lowest`.

## Why

- **F15 (PRD §4) requires a Scoop channel.** This manifest is the channel; the bucket repo (S2) hosts it; CI
  (P1.M5.T1.S2) pushes it. Per PRD §12 / external_deps.md §3, Scoop is "unaffected (they don't enforce
  code-signing)", so the unsigned Inno installer is fine here (unlike Winget's "unverified publisher" prompt).
- **Mirrors the proven AUR + Homebrew pattern.** AUR (`packaging/linux/aur/`) and Homebrew
  (`packaging/homebrew/`) already ship a manifest/formula + a source-repo README + (separately) a publish script.
  This task is the Scoop analogue: the manifest (≈ AUR `PKGBUILD` / Homebrew `Casks/qmkonnect.rb`) + the source-repo
  README (≈ `packaging/linux/aur/README.md` / `packaging/homebrew/README.md`).
- **`checkver`/`autoupdate` make it self-maintaining.** New GitHub tags are detected automatically; the bucket
  maintainer (or CI) runs `scoop checkup`/autoupdate and the manifest's `version`/`url`/`hash` regenerate with no
  hand-editing (hash computed from the download — no sidecar needed).

## What

### File 1 — `packaging/scoop/qmkonnect.json` (create verbatim)

> **Copy this block EXACTLY.** It has a SINGLE `"##"` comment key (the Scoop schema's documented comment field;
> the schema is `additionalProperties:false`, so a stray key like `"## "` (trailing space) would FAIL validation).
> 4-space indent; the comment string is one line (wrapped here only for readability — keep it as a single JSON
> string value, no embedded newline).

```json
{
    "##": "Scoop manifest for QMKonnect (Windows tray app). Consumes the per-tag GitHub-release Inno installer QMKonnect-<version>-windows-x64.exe. innosetup:true => Scoop EXTRACTS via innounp (installer logic does NOT run: no HKCU Run autostart, no Add/Remove-Programs entry, no AUMID on the Start Menu shortcut). See packaging/scoop/README.md. CI (P1.M5.T1.S2) fills the real SHA256 hash before publishing to the bucket.",
    "version": "0.2.8",
    "description": "Cross-platform window activity notifier for QMK keyboards (Windows tray app)",
    "homepage": "https://github.com/dabstractor/qmkonnect",
    "license": "MIT",
    "innosetup": true,
    "architecture": {
        "64bit": {
            "url": "https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe",
            "hash": "0000000000000000000000000000000000000000000000000000000000000000"
        }
    },
    "shortcuts": [
        [
            "QMKonnect",
            "QMKonnect.exe"
        ]
    ],
    "checkver": {
        "github": "https://github.com/dabstractor/qmkonnect",
        "regex": "v([\\d.]+)"
    },
    "autoupdate": {
        "architecture": {
            "64bit": {
                "url": "https://github.com/dabstractor/qmkonnect/releases/download/v$version/QMKonnect-$version-windows-x64.exe"
            }
        }
    }
}
```

**Field-by-field rationale** (all verified against `ScoopInstaller/Scoop/schema.json` this session):
- `version`: `"0.2.8"` — matches Cargo.toml; pattern `^[\w\.\-+_]+$`; NO leading `v` (the tag is `v0.2.8`, the
  URL path adds the `v`).
- `description`: from Cargo.toml `description` ("Cross-platform window activity notifier for QMK keyboards").
- `homepage`: `https://github.com/dabstractor/qmkonnect` (the source repo; matches Cargo `MyAppURL` + the
  release-URL host).
- `license`: `"MIT"` (Cargo.toml `license`). SPDX identifier — schema-valid.
- `innosetup`: `true` (boolean, no quotes) — tells Scoop to extract the Inno installer via `innounp` instead of
  running it. ⇒ the installer's HKCU Run autostart / Start Menu / ARP entry / AUMID post-install DO NOT run; Scoop
  manages the Start Menu shortcut via `shortcuts` (see Differences in the README).
- `architecture.64bit.{url,hash}`: the release is **x64-only** (`ArchitecturesAllowed=x64compatible`), so a
  `64bit` block is the correct, explicit form (cleaner than a bare top-level url for an arch-specific build).
  - `url`: the v0.2.8 release asset `QMKonnect-0.2.8-windows-x64.exe`.
  - `hash`: **64 zeros** — schema-valid (`^([a-fA-F0-9]{64}|…)$`) AND safe (Scoop checks the manifest hash
    against the downloaded file's computed hash at install; zeros fail that check, blocking install until CI
    fills the real SHA256). This is the documented "placeholder, CI fills it" pattern (mirrors the AUR PKGBUILD's
    publish-time hash + the Homebrew cask's `:no_check`→patched hash). **Do NOT** ship the real install with zeros.
- `shortcuts`: `[["QMKonnect","QMKonnect.exe"]]` — Scoop creates a Start Menu "QMKonnect" shortcut pointing to the
  extracted exe. **`QMKonnect.exe`** (capital Q) is the Inno `DestName` (`MyAppExeName`); Windows FS is
  case-insensitive so `qmkonnect.exe` also resolves, but `QMKonnect.exe` is the canonical name.
- `checkver`: `{ "github": "https://github.com/dabstractor/qmkonnect", "regex": "v([\\d.]+)" }` — Scoop scrapes
  the repo's latest release tag and captures the bare version (`v0.2.8` → `0.2.8`). `\\d` is double-escaped
  (JSON string → regex `\d`).
- `autoupdate.architecture.64bit.url`: the SAME url template with `$version` substituted. **`hash` is intentionally
  OMITTED** — Scoop COMPUTES the SHA256 from the download on `scoop checkup`/autoupdate (standard for
  GitHub-release apps; CI publishes NO `.sha256` sidecar — confirmed: `grep sha256|sidecar release.yml` → none).

### File 2 — `packaging/scoop/README.md` (author section-by-section)

Model the structure/tone on `packaging/homebrew/README.md` (title+oneliner → What this is → Install → What it
installs → Differences → maintenance → maintainers → cross-links). Author these sections:

1. **Title + one-line**: `# qmkonnect — Scoop manifest (Windows)` — Scoop app manifest for
   [QMKonnect](https://github.com/dabstractor/qmkonnect), the Windows community channel (PRD §4 F15, §5) alongside
   the primary Inno `.exe` direct installer.
2. **What this is**: a Scoop **app manifest** (`qmkonnect.json`) — not a formula. It downloads the per-tag GitHub
   release **Inno installer** `QMKonnect-<version>-windows-x64.exe` (the `windows` job in
   `.github/workflows/release.yml`, renamed from `QMKonnect-Setup.exe`) and **extracts** it via Scoop's
   `innosetup: true` (`innounp`). **No Rust toolchain, no `cargo`, no build dependencies** — the release exe
   statically links the CRT (`+crt-static`) and runs on any clean Windows 10/11 x64 box. Scoop is **per-user**, so
   the install needs no admin (matches the installer's `PrivilegesRequired=lowest`). x64-only. **Not code-signed**
   — fine for Scoop (PRD §12: "Scoop unaffected, they don't enforce code-signing").
3. **Install** (the EXACT bucket+install commands):
   ```bash
   # Add the bucket, then install:
   scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet
   scoop install qmkonnect
   # Update to the latest release:
   scoop update qmkonnect
   # Uninstall:
   scoop uninstall qmkonnect
   ```
   Note `scoop bucket add qmkonnect` alone resolves to the implicit user bucket — the explicit URL is REQUIRED
   (the bucket repo is `dabstractor/scoop-qmkonnet`, owned by the `dabstractor` org). State that `scoop update`
   pulls new releases automatically (the manifest's `checkver`/`autoupdate` detect GitHub tags).
4. **What it installs** (table):
   | Where | What |
   |---|---|
   | `~\scoop\apps\qmkonnect\current\QMKonnect.exe` | The tray-app binary (extracted from the Inno installer) |
   | `…\current\Icon.ico`, `…\IconTray-dark.png` | Icon assets (extracted alongside the exe) |
   | Start Menu → "QMKonnect" | Scoop-managed shortcut (the manifest's `shortcuts`) |
   | `%APPDATA%\QMKonnect\{config.toml,rules.toml}` | Per-user config (app-managed, NOT under the Scoop tree) |
5. **Differences from the direct Inno installer** (CRITICAL — this is the `innosetup:true` extraction delta). State
   plainly: **Scoop EXTRACTS the installer; it does not run it.** Therefore, relative to double-clicking
   `QMKonnect-Setup.exe`:
   - **Autostart is NOT on by default.** The Inno installer writes the HKCU `Run` value `QMKonnect` (default-on);
     extraction does not. → Enable **"Open at Login"** in QMKonnect's tray menu (the app writes the SAME HKCU Run
     value, keyed to the current exe path — works correctly from the Scoop tree). (Source: `src/autostart.rs`.)
   - **No Add/Remove-Programs entry.** Manage the app with `scoop uninstall qmkonnect` (and `scoop update`), not
     "Apps & features".
   - **Start Menu shortcut has no AppUserModelID.** The Inno installer runs `set_aumid.ps1` to brand WinRT toast
     notifications as "QMKonnect" (`Mulletware.QMKonnect`); extraction skips that step, so toasts (P1.M4) render
     generically until a future manifest `post_install` sets it. (Documented enhancement, out of scope here.)
   - **Location.** The app lives in the Scoop apps tree (`~\scoop\apps\qmkonnect\current\`), not
     `%LOCALAPPDATA%\Programs\QMKonnect\` (the Inno `{app}`). Config is unaffected (per-user `%APPDATA%`).
6. **Version & hash maintenance**: the manifest's `version`/`url`/`hash` are regenerated mechanically — `scoop
   checkup` (or the bucket's autoupdate) detects new GitHub tags via `checkver` and fills `version`+`url` from
   `autoupdate` (the SHA256 is computed from the download; the release publishes no `.sha256` sidecar). CI
   (P1.M5.T1.S2) does this automatically on each tag and pushes the manifest to the bucket. The shipped manifest
   carries a 64-zero `hash` placeholder; **CI fills the real SHA256 before publishing** (a zero hash fails Scoop's
   install-time hash check, so an unfilled manifest can never silently install).
7. **For maintainers** (the bucket repo + CI pointers — DO NOT create these here, just document):
   - Bucket repo: [`dabstractor/scoop-qmkonnet`](https://github.com/dabstractor/scoop-qmkonnet) (S2 owns its
     README + the `update-manifest.sh` publish script — the Scoop analogue of `packaging/homebrew/update-cask.sh`).
   - CI publish: on a tag, the release workflow (P1.M5.T1.S2) clones the bucket via a **deploy key**, runs the
     autoupdate to refresh `version`/`url`/`hash`, commits, pushes. (Mirrors the AUR SSH-key + Homebrew deploy-key
     model — see `architecture/external_deps.md` §"CI Publishing Strategy".)
8. **Cross-links**: the source repo (`https://github.com/dabstractor/qmkonnect`), `docs/installation.md` (Windows
   section), `packaging/windows/inno/` (the Inno installer this consumes), `spec/PACKAGING.md` §3 (Windows
   packaging), and the sibling manifests (`packaging/homebrew/README.md`, `packaging/linux/aur/README.md`).

### Success Criteria
- [ ] `packaging/scoop/qmkonnect.json` exists and matches the structure in File 1 (single `"##"` comment; `64bit`
      architecture block; `shortcuts`; `checkver{github,regex}`; `autoupdate.architecture.64bit.url` with `$version`).
- [ ] `packaging/scoop/README.md` exists with sections 1–8; contains the exact
      `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet` + `scoop install qmkonnect`.
- [ ] `jq . packaging/scoop/qmkonnect.json` → valid JSON (0 exit).
- [ ] The manifest validates against the authoritative Scoop schema (required fields, hash 64-hex pattern,
      shortcuts 2-element array, checkver object form, autoupdate.url carries `$version`).
- [ ] No edit to any Rust source / Cargo.toml / `.github/workflows/*` / other packaging dir / docs outside
      `packaging/scoop/`; no publish script created (that is P1.M3.T1.S2).

## All Needed Context

### Context Completeness Check
_Pass_: an agent with no prior Scoop knowledge can create both files verbatim from the "What" section (the manifest
JSON is given in full; the README is specced section-by-section with exact commands), then validate the JSON with
`jq` + the authoritative schema (`schema.json` downloaded once) — all using only this PRP + the codebase. The
Windows-only `scoop install`/`checkver` smoke test is explicitly deferred to a Windows host.

### Documentation & References

```yaml
# MUST READ — authoritative Scoop manifest spec + schema (external)
- url: https://github.com/ScoopInstaller/Scoop/wiki/App-Manifests
  why: the manifest field reference — version/url/hash/innosetup/shortcuts/checkver/autoupdate semantics
  critical: "innosetup:true => Scoop EXTRACTS via innounp (does NOT run the installer); shortcuts are Scoop-managed;
            autoupdate.url uses the $version variable; if autoupdate omits hash, Scoop COMPUTES it from the download"
- url: https://github.com/ScoopInstaller/Scoop/wiki/App-Manifest-Autoupdate
  why: the checkver/autoupdate mechanics the manifest's blocks enable; $version/$baseurl variables; hash auto-compute
- url: https://github.com/ScoopInstaller/Scoop/blob/master/schema.json
  why: the AUTHORITATIVE JSON schema to validate against (verified this session) — required=[version,homepage,license]+url;
       hash pattern ^([a-fA-F0-9]{64}|(sha…):…)$; shortcuts items minItems:2 maxItems:4; checkver={github,regex} object form
  critical: "a 64-zero hash PASSES the schema AND is install-blocking (safe placeholder). checkver.regex is a JSON string
            → escape backslashes (\"\\\\d\" in-file => regex \\d)."

# MUST READ — the architecture decision this implements
- docfile: plan/007_fb356ba503b4/architecture/external_deps.md
  why: §3 Scoop — package type (app manifest JSON), key file (qmkonnect.json), publication (custom bucket
       `scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet`), per-user, required fields
       (version/url/hash/shortcuts/checkver/autoupdate, innosetup:true), CI (push manifest to bucket on tag),
       signing note (Scoop doesn't enforce code-signing). §"CI Publishing Strategy" (deploy key, clone→update→push).
       §"Version Source of Truth" (Cargo.toml via cargo metadata) + §"Hashing" (Scoop: hash in manifest).
  section: "3. Scoop (Windows)" + "CI Publishing Strategy" + "Version Source of Truth" + "Hashing"

# MUST READ — PRD context (the feature + platform row this is the channel for)
- url: spec/PRD.md
  why: §4 F15 (community package-manager distribution: AUR/Homebrew/Scoop/Winget/Nix/mise/asdf);
       §5 platform row "Windows: Inno .exe (primary, no admin) · Scoop · Winget"; §12 signing note
  section: "h2.3 (4. Top-Level Feature Set, F15)" + "h2.4 (5. Supported Platforms)" + "h2.74/h3.109 (3. Windows Packaging)"

# MUST READ — the Inno installer this manifest consumes (what innounp extracts vs what does NOT run)
- file: packaging/windows/inno/QMKonnect.iss
  why: confirms the release-asset payload + the installer logic that innosetup:true SKIPS
  pattern: "DestName QMKonnect.exe (= MyAppExeName); {app}={localappdata}\\Programs\\QMKonnect; files qmkonnect.exe
            (as QMKonnect.exe)+Icon.ico+IconTray-dark.png; [Registry] HKCU Run 'QMKonnect' autostart; [Icons] Start Menu;
            [Code]CurStepChanged runs set_aumid.ps1 → Mulletware.QMKonnect; ArchitecturesAllowed=x64compatible;
            PrivilegesRequired=lowest (per-user, no admin)"
  gotcha: "innosetup:true extracts the {app} files; it does NOT run [Registry]/[Icons]/[Code]. So under Scoop:
           autostart is off-by-default, no ARP entry, no AUMID on the shortcut. Document these in the README."

# MUST READ — the precedent to mirror (AUR manifest + README consuming a GitHub release artifact)
- file: packaging/linux/aur/README.md
  why: the established README structure/tone (title+oneliner → what-this-is → install → what-it-installs →
       version&checksum maintenance → maintainers → cross-links). The Scoop README follows the same skeleton.
- file: packaging/linux/aur/PKGBUILD
  why: the established "manifest consuming the GitHub release artifact, hash filled at publish, version from Cargo"
       pattern. The Scoop manifest is the Windows analogue (Inno .exe instead of the linux tarball).
  gotcha: "AUR's PKGBUILD has a REAL sha256 for 0.2.8 (the linux tarball). The Scoop manifest CANNOT reuse that hash
           (different artifact). It ships a 64-zero placeholder; CI fills the Windows .exe SHA256."

# MUST READ — the Homebrew precedent (tap-README + update script split = the S1/S2 boundary model)
- file: packaging/homebrew/README.md
  why: the macOS analogue of THIS file (a source-repo packaging doc for a community channel). Tone + section model.
- docfile: plan/007_fb356ba503b4/P1M2.T1.S2/PRP.md   (parallel, in-flight)
  why: the CONTRACT for the Homebrew tap-README + update-cask.sh (S2). The Scoop S1/S2 split mirrors it EXACTLY:
       THIS task = manifest + source README (≈ Homebrew S1's cask + packaging/homebrew/README.md);
       P1.M3.T1.S2 = bucket-README + update-manifest.sh (≈ Homebrew S2's tap-README + update-cask.sh). Do NOT write
       the publish script here. No file overlap (packaging/scoop/ vs packaging/homebrew/).
  critical: "the deploy-key CI model, the version v-prefix rule, and the 'script does local update + documents the
             key; CI does the git push' scope split all carry over to the Scoop S2."

# MUST READ — release-artifact facts (verified in release.yml)
- file: .github/workflows/release.yml
  why: the `windows` job builds QMKonnect-Setup.exe (Inno) and renames it to
       `QMKonnect-<version>-windows-x64.exe` (the release asset); version via `cargo metadata|ConvertFrom-Json`
       (no `v` prefix); the `publish` job creates the GitHub Release with `softprops/action-gh-release@v2`.
  gotcha: "version has NO leading v; the URL path uses v<version>; the asset filename uses the bare version.
           NO .sha256 sidecar is published (grep sha256|sidecar → none) ⇒ autoupdate must COMPUTE the hash."

# REFERENCE — naming facts (consistent with Homebrew/AUR)
- file: Cargo.toml
  why: version=0.2.8, license=MIT, description, authors=["Mulletware"] — the manifest's metadata source of truth
- file: src/platforms/mod.rs:138  (APP_AUMID = "Mulletware.QMKonnect")
  why: the AUMID the Inno installer sets on the Start Menu .lnk (and that innosetup:true extraction skips)
- url: https://github.com/dabstractor/scoop-qmkonnet   (the bucket repo — S2 owns it; this task only references it)
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/
  windows/inno/QMKonnect.iss        # the Inno installer the manifest consumes (READ for payload + skipped logic)
  windows/inno/build.ps1            # builds QMKonnect-Setup.exe (version from Cargo.toml)
  linux/aur/{PKGBUILD,README.md,publish.sh}   # the MANIFEST+README+PUBLISH-SCRIPT precedent (S1+S2 model)
  homebrew/{Casks/qmkonnect.rb,README.md}     # the macOS analogue of THIS task (cask + source README)
  homebrew/{tap-README.md,update-cask.sh}     # (P1.M2.T1.S2, parallel) the S2 analogue this task must NOT duplicate
.github/workflows/release.yml       # windows job → QMKonnect-<ver>-windows-x64.exe; publish job; NO sha256 sidecar
Cargo.toml                          # version/license/description metadata source
# NEW (this task):
packaging/scoop/
  qmkonnect.json                    # the Scoop app manifest
  README.md                         # source-repo packaging doc (Mode A)
```

### Desired Codebase tree (files this task ADDS)

```bash
packaging/scoop/
├── qmkonnect.json   # Scoop app manifest (version/url/hash/innosetup/shortcuts/checkver/autoupdate)
└── README.md        # source-repo packaging doc (what/install/what-it-installs/differences/maintenance/maintainers)
```
(No other files. The bucket-repo README + `update-manifest.sh` = P1.M3.T1.S2; the CI push job = P1.M5.T1.S2.)

### Known Gotchas of our codebase & Library Quirks
```bash
# CRITICAL (innosetup:true = EXTRACT, not RUN): Scoop uses innounp to extract the Inno {app} payload into the
#   Scoop apps tree. The installer's [Registry] HKCU Run autostart, [Icons] Start Menu, Add/Remove-Programs entry,
#   and [Code]CurStepChanged AUMID step DO NOT run. Scoop creates the Start Menu shortcut itself via `shortcuts`.
#   ⇒ under Scoop: autostart is OFF by default (use the tray "Open at Login" toggle), no ARP entry (use scoop
#   uninstall), no AUMID on the shortcut (toasts render generically until a future post_install — out of scope).
#   Document ALL of this in the README's "Differences from the direct installer" section.

# CRITICAL (NO .sha256 sidecar): grep sha256|sidecar .github/workflows/release.yml → NOTHING. The release publishes
#   only the renamed .exe. So the autoupdate block must OMIT hash (Scoop computes it from the download). Do NOT
#   invent a hash.url sidecar that doesn't exist. If a sidecar is added later, a hash.url entry can be added then.

# CRITICAL (hash placeholder shape): the schema hashPattern requires EXACTLY 64 hex chars (or a (sha…): prefix).
#   Use 64 zeros ("000…0"). This is schema-valid AND install-blocking (Scoop checks manifest hash vs computed hash
#   at install → zeros fail until CI fills the real SHA256). A non-hex placeholder like "PLACEHOLDER" FAILS the
#   schema. CI (P1.M5.T1.S2) fills the real hash before publishing.

# CRITICAL (version v-prefix): version has NO leading v ("0.2.8"); the tag is "v0.2.8"; the URL path adds the v
#   (.../v0.2.8/QMKonnect-0.2.8-windows-x64.exe); the asset filename uses the bare version. autoupdate.url uses
#   $version (bare) so it renders ".../v$version/QMKonnect-$version-windows-x64.exe". Do NOT prepend v to $version.

# CRITICAL (JSON regex escaping): checkver.regex is a JSON string. `\d` in the regex must be written "\\d" in the
#   JSON file (one backslash for JSON → one for the regex engine). Same for any other backslash metachar.

# CRITICAL (bucket alias != org): the bucket repo is dabstractor/scoop-qmkonnet (org dabstractor). The install
#   command `scoop bucket add qmkonnect <url>` uses the alias "qmkonnect" WITH the explicit URL — `scoop bucket add
#   qmkonnect` alone looks for an implicit user bucket (wrong). Use the exact command from the contract in the README.

# CRITICAL (exe name): the Inno installer renames the built qmkonnect.exe → QMKonnect.exe (DestName: MyAppExeName).
#   innounp preserves DestName, so the extracted file is QMKonnect.exe. shortcuts references "QMKonnect.exe"
#   (Windows FS is case-insensitive, but the canonical name is capitalized). Do NOT use a lowercase path.

# GOTCHA (x64-only): the release is x64-only (ArchitecturesAllowed=x64compatible). Use a `64bit` architecture block
#   rather than a bare top-level url — explicit and matches the arch-specific build. (A bare top-level url would
#   also work but is less precise for an x64-only artifact.)

# GOTCHA (no bin shim): a tray app's entry point is the Start Menu shortcut, not a CLI. The contract does not list
#   `bin`, so omit it. (Adding `bin` would create a console shim that flashes a window on launch — undesirable for
#   a windows_subsystem="windows" tray app.)

# GOTCHA (scope): do NOT add a post_install autostart/AUMID script here — it reaches into P1.M4 (toast/AUMID) scope,
#   can't be tested on the Linux dev box, and is not in the contract's field list. Document the deltas instead.
#   Do NOT write a publish/bucket-update script (that is P1.M3.T1.S2's update-manifest.sh).

# GOTCHA (scoop is Windows-only): the dev box is Linux. `scoop install`/`scoop checkver` can't run here. Validate
#   the manifest with `jq` + the authoritative schema (Linux-validatable); defer the install smoke test to a Windows
#   host (note in the report), same platform-split as the Homebrew/AUR target-OS deferrals.
```

## Implementation Blueprint

### Data models and structure
No code models. Two static files: a JSON manifest (declares the install recipe + version/hash autoupdate) and a
Markdown doc. The manifest references no types/structs; CI patches its scalar `version`/`url`/`hash` per release.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/scoop/qmkonnect.json
  - IMPLEMENT: copy the JSON from "What → File 1" VERBATIM. It has a SINGLE "##" comment key (the schema's
    documented comment field; additionalProperties:false means a stray key like "## " would FAIL validation).
    The comment string is ONE JSON string value (no embedded newline). Use the 64-zero hash placeholder.
  - STRUCTURE: ##, version, description, homepage, license, innosetup (true), architecture.64bit.{url,hash},
    shortcuts ([["QMKonnect","QMKonnect.exe"]]), checkver {github, regex:"v([\\d.]+)"},
    autoupdate.architecture.64bit.url (v$version template; NO hash — auto-computed).
  - FOLLOW pattern: schema keys + ordering match ScoopInstaller/Scoop/schema.json (verified). 4-space indent.
  - PLACEMENT: packaging/scoop/qmkonnect.json (create the packaging/scoop/ dir).

Task 2: VALIDATE the manifest (no Windows needed for these)
  - RUN: jq . packaging/scoop/qmkonnect.json                               → valid JSON (exit 0)
  - RUN (schema): download the authoritative schema once and validate —
        curl -fsSL https://raw.githubusercontent.com/ScoopInstaller/Scoop/master/schema.json -o /tmp/scoop-schema.json
        # then with a JSON-schema validator available on the box:
        python -m jsonschema -i packaging/scoop/qmkonnect.json /tmp/scoop-schema.json   # if jsonschema installed
        #   OR:  ajv validate -s /tmp/scoop-schema.json -d packaging/scoop/qmkonnect.json  # if ajv(npm) installed
    EXPECT: valid (required version/homepage/license+url; hash 64-hex; shortcuts 2-elem; checkver object; autoupdate.url $version).
    If NO validator is installed, fall back to the grep assertions below (they pin the schema-critical bits).
  - RUN (grep assertions — Linux-safe, no deps):
        jq -e '.version=="0.2.8" and .license=="MIT" and .innosetup==true' packaging/scoop/qmkonnect.json
        jq -e '.architecture."64bit".url|test("v0\\.2\\.8/QMKonnect-0\\.2\\.8-windows-x64\\.exe$")' packaging/scoop/qmkonnect.json
        jq -e '.architecture."64bit".hash|test("^[0-9a-fA-F]{64}$")' packaging/scoop/qmkonnect.json
        jq -e '.shortcuts[0]==["QMKonnect","QMKonnect.exe"]' packaging/scoop/qmkonnect.json
        jq -e '.checkver.github=="https://github.com/dabstractor/qmkonnect" and (.checkver.regex|test("^v"))' packaging/scoop/qmkonnect.json
        jq -e '.autoupdate.architecture."64bit".url|test("v\\$version/QMKonnect-\\$version-windows-x64")' packaging/scoop/qmkonnect.json
    EXPECT: every `jq -e` prints `true` (exit 0).
  - NOTE: `scoop install`/`scoop checkver` are Windows-only → DEFER to a Windows host; do not attempt on Linux.

Task 3: CREATE packaging/scoop/README.md
  - IMPLEMENT: sections 1–8 from "What → File 2". Title `# qmkonnect — Scoop manifest (Windows)`.
  - MUST INCLUDE verbatim:
      scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet
      scoop install qmkonnect
      scoop update qmkonnect
      scoop uninstall qmkonnect
    AND the "Differences from the direct Inno installer" bullets (autostart off-by-default → tray toggle; no ARP
    entry → scoop uninstall; no AUMID on shortcut → toasts generic until a future post_install; Scoop-tree location).
    AND the maintainer pointers (bucket dabstractor/scoop-qmkonnet → S2's update-manifest.sh; CI deploy key → P1.M5.T1.S2).
  - FOLLOW pattern: packaging/homebrew/README.md (and packaging/linux/aur/README.md) structure/tone.
  - PLACEMENT: packaging/scoop/README.md.

Task 4: NEVER do these (out of scope / forbidden)
  - DO NOT write a publish/autoupdate SCRIPT (update-manifest.sh) or a bucket-repo README — that is P1.M3.T1.S2.
  - DO NOT edit .github/workflows/* (the CI push job is P1.M5.T1.S2; this task only DOCUMENTS it).
  - DO NOT add a post_install autostart/AUMID script to the manifest (reaches into P1.M4; untestable on Linux;
    not in the contract). Document the deltas in the README instead.
  - DO NOT add `bin` (console shim) — a tray app uses the Start Menu shortcut, not PATH.
  - DO NOT invent a .sha256 sidecar hash.url (none is published; autoupdate computes the hash).
  - DO NOT use a non-hex hash placeholder (schema requires 64 hex); use 64 zeros and let CI fill it.
  - DO NOT prepend `v` to version or to $version (tag is v-prefixed; version/url-filename are not).
  - DO NOT change any Rust source / Cargo.toml / other packaging dir / docs outside packaging/scoop/.
  - DO NOT edit PRD.md, any tasks.json, or prd_snapshot.md.
```

### Implementation Patterns & Key Details
```jsonc
// The manifest skeleton (see File 1 for the full verbatim JSON). Key invariants:
{
  "version": "0.2.8",                 // NO leading v; pattern ^[\w\.\-+_]+$
  "license": "MIT",                   // SPDX identifier (Cargo.toml)
  "innosetup": true,                  // boolean => Scoop EXTRACTS via innounp (does not run the installer)
  "architecture": {
    "64bit": {                         // x64-only release
      "url": "https://github.com/dabstractor/qmkonnect/releases/download/v0.2.8/QMKonnect-0.2.8-windows-x64.exe",
      "hash": "0000…0"                 // 64 zeros: schema-valid + install-blocking (CI fills the real SHA256)
    }
  },
  "shortcuts": [["QMKonnect", "QMKonnect.exe"]],   // Scoop-managed Start Menu shortcut (Inno's [Icons] does NOT run)
  "checkver": { "github": "https://github.com/dabstractor/qmkonnect", "regex": "v([\\d.]+)" },  // \\d => \d in-file
  "autoupdate": {
    "architecture": { "64bit": { "url": ".../v$version/QMKonnect-$version-windows-x64.exe" } }  // hash OMITTED => computed
  }
}
```
```text
# PATTERN: mirror the AUR/Homebrew manifest+source-README+separate-publish-script split.
#   THIS task  = manifest (qmkonnect.json) + source README  ≈  AUR PKGBUILD + aur/README.md  ≈  Homebrew cask + homebrew/README.md
#   P1.M3.T1.S2 = bucket-README + update-manifest.sh        ≈  aur/publish.sh              ≈  Homebrew tap-README + update-cask.sh
#   P1.M5.T1.S2 = CI deploy-key push                        ≈  AUR CI (P1.M5.T1.S1)        ≈  Homebrew CI (P1.M5.T1.S2)

# PATTERN (hash): 64-zero placeholder == the "template; CI fills it" idiom. AUR ships a REAL hash for 0.2.8 (linux
#   tarball); Scoop CANNOT reuse it (different artifact) → zeros + CI fill. Safe: zeros fail Scoop's install-time
#   hash check, so an unfilled manifest can never silently install a tampered binary.

# ANTI-PATTERN: don't add hash.url to autoupdate pointing at a .sha256 sidecar — none is published. Omit hash;
#   Scoop computes it. (A future sidecar can add hash.url later without restructuring.)
```

### Integration Points
```yaml
INPUT (release):    QMKonnect-<version>-windows-x64.exe (release.yml windows job, renamed from QMKonnect-Setup.exe)
OUTPUT:             packaging/scoop/{qmkonnect.json, README.md}
BUCKET REPO:        dabstractor/scoop-qmkonnet (S2 owns its README + update-manifest.sh)
CI (P1.M5.T1.S2):   on tag → autoupdate refreshes version/url/hash (deploys the deploy key) → clones bucket → cp manifest → commit → push
METADATA SOURCE:    Cargo.toml (version=0.2.8, license=MIT, description) — single source of truth (external_deps.md §"Version Source of Truth")
DOCS SYNC (P1.M6):  docs/installation.md Windows section + top-level README will link the bucket (NOT this task)
PARALLEL (no conflict):
  - P1.M2.T1.S2 (Homebrew tap, in-flight): packaging/homebrew/* — different dir, pattern precedent only.
  - P1.M3.T1.S2 (Scoop bucket+script): owns the bucket-repo README + update-manifest.sh; THIS task owns the
    manifest + source README. The manifest's checkver/autoupdate blocks are what S2's automation consumes.
PLATFORM VALIDATION: Linux box validates JSON (jq) + schema + grep assertions. `scoop install`/`checkver`
  are Windows-only → deferred to a Windows host (note in report).
```

## Validation Loop

> Toolchain: the manifest is plain JSON; `jq` + a JSON-schema validator run on Linux. Scoop itself is Windows-only.

### Level 1: JSON validity (Linux)
```bash
cd /home/dustin/projects/qmkonnect
jq . packaging/scoop/qmkonnect.json >/dev/null && echo "valid JSON"
# Expected: "valid JSON" (exit 0). If "parse error", fix the JSON (most likely: a stray "## " key, or an
# unescaped backslash in checkver.regex — \d must be \\d in-file).
```

### Level 2: Schema conformance + field assertions (Linux)
```bash
cd /home/dustin/projects/qmkonnect
# Download the authoritative schema once:
curl -fsSL https://raw.githubusercontent.com/ScoopInstaller/Scoop/master/schema.json -o /tmp/scoop-schema.json
# Validate with whichever validator is on PATH:
if command -v ajv >/dev/null; then ajv validate -s /tmp/scoop-schema.json -d packaging/scoop/qmkonnect.json
elif python -c 'import jsonschema' 2>/dev/null; then
    python -m jsonschema -i packaging/scoop/qmkonnect.json /tmp/scoop-schema.json
else echo "(no jsonschema/ajv installed — relying on the jq assertions below)"; fi
# Pin the schema-critical bits (run regardless of validator availability):
jq -e '.version=="0.2.8" and .license=="MIT" and ((.homepage)|startswith("https://github.com/dabstractor/qmkonnect")) and .innosetup==true' packaging/scoop/qmkonnect.json
jq -e '.architecture."64bit".url|test("^https://github.com/dabstractor/qmkonnect/releases/download/v0\\.2\\.8/QMKonnect-0\\.2\\.8-windows-x64\\.exe$")' packaging/scoop/qmkonnect.json
jq -e '.architecture."64bit".hash|test("^[0-9a-fA-F]{64}$")' packaging/scoop/qmkonnect.json
jq -e '.shortcuts[0]==["QMKonnect","QMKonnect.exe"]' packaging/scoop/qmkonnect.json
jq -e '.checkver.github=="https://github.com/dabstractor/qmkonnect"' packaging/scoop/qmkonnect.json
jq -e '.checkver.regex=="v([\\d.]+)"' packaging/scoop/qmkonnect.json
jq -e '.autoupdate.architecture."64bit".url|test("v\\$version/QMKonnect-\\$version-windows-x64\\.exe$")' packaging/scoop/qmkonnect.json
# Expected: every `jq -e` prints `true` (exit 0). (Note: autoupdate must NOT have a hash key — verify its absence:
#   `jq -e 'has("hash")|not' <(jq '.autoupdate.architecture."64bit"' packaging/scoop/qmkonnect.json)` → true.)
```

### Level 3: Content review (Linux)
```bash
cd /home/dustin/projects/qmkonnect
grep -nE 'scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet|scoop install qmkonnect|scoop update qmkonnect|scoop uninstall qmkonnect|Open at Login|innounp|set_aumid|Mulletware.QMKonnect|dabstractor/scoop-qmkonnet|P1\.M3\.T1\.S2|P1\.M5\.T1\.S2' packaging/scoop/README.md
# Expected: hits for the bucket+install+update+uninstall commands; the autostart ("Open at Login") + AUMID
# deltas; the innounp-extraction note; the bucket repo + S2/CI pointers.
git diff --stat     # Expected: ONLY packaging/scoop/{qmkonnect.json,README.md} (new).
git diff Cargo.toml .github/workflows/release.yml   # Expected: empty.
```

### Level 4: Scoop smoke test (Windows host — OPTIONAL, deferred)
```powershell
# On a Windows box with Scoop installed, AFTER CI has filled the real hash + published the manifest to the bucket:
scoop bucket add qmkonnect https://github.com/dabstractor/scoop-qmkonnet
scoop install qmkonnect
# Expected: downloads QMKonnect-<ver>-windows-x64.exe, extracts via innounp, places QMKonnect.exe + icons in
# ~\scoop\apps\qmkonnect\current\, creates a Start Menu "QMKonnect" shortcut. Launch it; enable "Open at Login".
# (SKIPPABLE on the Linux dev box — `scoop` is Windows-only. Note the deferral in the report.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `jq . packaging/scoop/qmkonnect.json` → valid JSON (exit 0).
- [ ] Manifest validates against the authoritative Scoop schema (or all `jq -e` field assertions print `true`).
- [ ] `git diff --stat` shows ONLY `packaging/scoop/qmkonnect.json` + `packaging/scoop/README.md`.

### Feature Validation
- [ ] Manifest has: version (no v), description, homepage, license (MIT), innosetup (true), 64bit architecture
      {url, 64-zero hash}, shortcuts ([["QMKonnect","QMKonnect.exe"]]), checkver {github, regex}, autoupdate
      (64bit.url with $version, NO hash key).
- [ ] README has the exact bucket+install+update+uninstall commands and the "Differences from the direct Inno
      installer" deltas (autostart off-by-default, no ARP entry, no AUMID on shortcut, Scoop-tree location).
- [ ] README points to the bucket repo (dabstractor/scoop-qmkonnet → S2's update-manifest.sh) and the CI deploy
      key (P1.M5.T1.S2); does NOT create those.

### Code Quality Validation
- [ ] Manifest mirrors the AUR/Homebrew manifest+source-README pattern; follows the Scoop schema field ordering.
- [ ] No publish/bucket-update script written (scope split respected — that is P1.M3.T1.S2).
- [ ] No `bin` console shim; no `post_install` autostart/AUMID (out of contract + P1.M4 scope); deltas documented.
- [ ] Naming consistent: org `dabstractor`, bucket `dabstractor/scoop-qmkonnet`, exe `QMKonnect.exe`, AUMID `Mulletware.QMKonnect`.

### Documentation & Deployment
- [ ] README documents install/update/uninstall, what-it-installs, the innounp-extraction differences, version &
      hash maintenance (CI fills the 64-zero placeholder), and the maintainer/bucket/CI pointers.
- [ ] Report notes that `scoop install`/`checkver` are Windows-only and were deferred to a Windows host (validated
      via `jq` + schema on the Linux box).

---

## Anti-Patterns to Avoid
- ❌ Don't run the Inno installer under Scoop — `innosetup:true` EXTRACTS it via `innounp`. The installer's HKCU Run
  autostart, Start Menu shortcut, ARP entry, and AUMID step do NOT fire. Document these deltas; don't pretend Scoop
  replicates the direct install bit-for-bit.
- ❌ Don't add a `hash.url` sidecar to autoupdate — CI publishes NO `.sha256` sidecar (verified). Omit `hash` from
  autoupdate; Scoop computes it from the download.
- ❌ Don't use a non-64-hex hash placeholder — the schema rejects it. Use 64 zeros (schema-valid + install-blocking
  until CI fills the real SHA256). Don't reuse the AUR tarball hash (different artifact).
- ❌ Don't prepend `v` to `version` or to the `$version` autoupdate variable — the tag is `v0.2.8`; the version/url
  filename are bare `0.2.8`; the URL path adds the `v` (`.../v$version/…`).
- ❌ Don't drop the explicit URL from `scoop bucket add qmkonnect <url>` — the alias ≠ org; without it Scoop looks
  for an implicit user bucket. Don't swap dabstractor↔scoop-qmkonnet.
- ❌ Don't write the publish/bucket-update script or the bucket-repo README here — that's P1.M3.T1.S2 (the Homebrew
  S2 split). This task = manifest + source README only.
- ❌ Don't add `bin` (a console shim) or a `post_install` autostart/AUMID script — both out of the contract's field
  list; `bin` is wrong for a windows_subsystem tray app; the post_install reaches into P1.M4 and can't be tested on
  Linux. Document, don't implement.
- ❌ Don't claim `scoop install`/`checkver` validation on a Linux box — Scoop is Windows-only. Validate with `jq` +
  the schema; defer the smoke test to Windows.
- ❌ Don't edit any Rust source / Cargo.toml / `.github/workflows/*` / other packaging dir / docs outside
  `packaging/scoop/`, or PRD.md / tasks.json / prd_snapshot.md.

---

## Confidence Score: 9/10

Both files are fully specified (the manifest JSON is given verbatim; the README is specced section-by-section with
exact commands + the innounp-extraction delta). Every schema-critical fact is verified this session against the
authoritative `ScoopInstaller/Scoop/schema.json` (required fields, 64-hex hash pattern, shortcuts 2-element array,
`{github,regex}` checkver, `$version` autoupdate, hash-omitted ⇒ computed), the release workflow (asset name
`QMKonnect-<ver>-windows-x64.exe`, no `v` in version, NO `.sha256` sidecar), the Inno installer internals
(`DestName=QMKonnect.exe`, HKCU Run autostart, AUMID `Mulletware.QMKonnect`, x64-only), and the naming facts
(dabstractor org, scoop-qmkonnet bucket). The Linux box validates JSON + schema + grep assertions; only the
Windows-only `scoop install`/`checkver` smoke test is deferred (explicitly skippable). The 1-point reservation is
for (a) the `innosetup:true` extraction behavior being assertable only by a real `scoop install` on Windows (the
Linux box proves the manifest is well-formed, not that innounp extracts cleanly), and (b) the 64-zero hash being a
CI-fill placeholder — both are standard, documented, low-risk, and explicitly called out as deferred-to-Windows /
CI-fill.