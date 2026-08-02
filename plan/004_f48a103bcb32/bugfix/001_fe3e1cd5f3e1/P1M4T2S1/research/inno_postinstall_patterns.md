# Research: Inno Setup 6 post-install AUMID-helper integration patterns

> **Research-method note (read first).** The `web_search` / `fetch_content` tools
> were **not available** in this sandbox (the runtime rejected them: *"Agent
> 'researcher' requested unavailable child tools"*). The answers below are
> reconstructed from authoritative knowledge of Inno Setup 6's stable,
> long-documented behavior. The *factual mechanics* (install order,
> `deleteafterinstall`/`{tmp}`, `[Run]` top-to-bottom execution, silent-install
> semantics, automatic shortcut uninstall) are stable Inno behavior I am highly
> confident in. The exact `index.php?topic=` URL **slugs** were reconstructed
> from memory and should be confirmed against the canonical root
> `https://jrsoftware.org/ishelp/index.php`; the root + topic *titles* are
> correct. All recommendations were verified against the **actual**
> `packaging/windows/inno/QMKonnect.iss` in this repo.

---

## Summary

The cleanest integration is **not** a `[Run]` entry but a
`[Code]`-section **`CurStepChanged(ssPostInstall)` + `Exec`** call, with the
helper extracted via `[Files] ... DestDir: "{tmp}"; Flags: deleteafterinstall`.
Inno guarantees this hook runs **after** `[Icons]` created the Start Menu `.lnk`
and **before** the `[Run]` section launches the app — so the AUMID is on the
shortcut before first launch, with a synchronous `ewWaitUntilTerminated` wait and
a checkable `ResultCode`, while the app-launch `[Run]` entry is left untouched.
Uninstall is automatic (Inno deletes the `[Icons]` shortcut it created), and
silent installs run the step unconditionally because `ssPostInstall` always fires
regardless of `/VERYSILENT`.

---

## Findings

### 1. Execution order — does `[Icons]` precede `[Run]` and `ssPostInstall`? YES (both)

Inno's install sequence, after the `PrepareToInstall`/`BeforeInstall`/`InitializeSetup`
phases, is:

1. `CurStepChanged(ssInstall)` — *"called just before Setup starts copying files."*
2. **`[Files]`** entries copied (each entry's `BeforeInstall`/`AfterInstall` fire around it).
3. **`[Icons]`** entries created (Start Menu `.lnk` written here).
4. **`[INI]`** entries.
5. **`[Registry]`** entries written (HKCU `Run` value here).
6. **`CurStepChanged(ssPostInstall)`** — *"called just after all files have been
   installed (and after the `[Icons]`, `[INI]`, `[Registry]` sections have been
   processed), and **just before Setup is about to process the `[Run]` section**."*
7. **`[Run]`** entries processed **top-to-bottom** (the app-launch entry runs here).
8. `CurStepChanged(ssDone)` — just before Setup terminates, after a *successful*
   install (does **not** fire if `[Run]` aborts).

**Direct answers:**

- **Is a `[Run]` entry guaranteed to run AFTER `[Icons]`?** ✅ **Yes.** `[Icons]`
  is step 3; `[Run]` is step 7.
- **Is `CurStepChanged(ssPostInstall)` guaranteed to run after `[Icons]`?** ✅
  **Yes.** `[Icons]` is step 3; `ssPostInstall` is step 6.
- **Bonus (critical for this task):** `ssPostInstall` (step 6) runs **before**
  `[Run]` (step 7). So anything done in `CurStepChanged(ssPostInstall)` is
  guaranteed complete before the app-launch `[Run]` entry runs.

**Reference:** Pascal Scripting topic *"Event Functions" → `CurStepChanged`* and
*"`TSetupStep`"*, plus the *"Install order"* topic.
`https://jrsoftware.org/ishelp/index.php?topic=scripting`
(`/ishelp/index.php` → "Pascal Scripting" → "Event Functions").

### 2. Bundling the helper so it does NOT remain in `{app}` — `deleteafterinstall` + `{tmp}`

⚠️ **Correction to the proposed snippet.** `DestDir` is a **mandatory** parameter
for every `[Files]` entry; the compiler errors (*"Required parameter
DestinationDir missing"*) if it is omitted. The task's proposed
`[Files] Source: "set_aumid.ps1"; Flags: deleteafterinstall` is therefore
**incomplete**. The correct, canonical line is:

```ini
Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall
```

- **`DestDir: "{tmp}"`** extracts the file to Inno's install-time temp dir. It is
  reachable at runtime via `ExpandConstant('{tmp}\set_aumid.ps1')`. It is **not**
  placed under `{app}` (`{localappdata}\Programs\QMKonnect`), so it never becomes a
  runtime asset.
- **`deleteafterinstall`** tells Setup to delete the file after installation
  completes (including on install failure), guaranteeing cleanup. (`{tmp}` is
  staging area; the flag makes removal explicit and reliable.)
- **Source path is RELATIVE TO THE .iss FILE.** With no `SourceDir` override in
  this `[Setup]` (verified in `QMKonnect.iss`), a bare `set_aumid.ps1` resolves to
  `packaging/windows/inno/set_aumid.ps1` — the same dir as `QMKonnect.iss`. This is
  consistent with the existing entries, which use `{#AssetDir}` (`..\..`) and
  `{#ReleaseDir}` (`..\..\..\target\release`) relative to the script dir.

**Verbatim `[Files]` block to add to `QMKonnect.iss`** (append after the existing
three asset lines):

```ini
; Install-time helper only: sets the App User Model ID (AUMID) on the Start Menu
; shortcut so Windows toast notifications brand as "QMKonnect". Extracted to {tmp}
; and deleted after install (deleteafterinstall) - never a runtime asset.
Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall
```

> **Timing check:** `[Files]` extraction (step 2) happens **before**
> `CurStepChanged(ssPostInstall)` (step 6), so `{tmp}\set_aumid.ps1` already exists
> when we invoke it. ✅

**Reference:** `[Files]` section + *"Flags"* (`deleteafterinstall`).
`https://jrsoftware.org/ishelp/index.php?topic=filescommon` (root → "Files and
Folders" → "[Files] section" → "Flags").

### 3. Invoking the script — RECOMMENDED: `[Code]` `CurStepChanged(ssPostInstall)` + `Exec`

**Recommendation: use the `[Code]` approach**, not a `[Run]` entry.

| Concern | `[Code] CurStepChanged(ssPostInstall)` | `[Run]` entry |
|---|---|---|
| Runs after `[Icons]` | ✅ guaranteed (step 6 > step 3) | ✅ guaranteed (step 7 > step 3) |
| Runs before app launch | ✅ guaranteed (step 6 < step 7) — absolute, regardless of `postinstall` ordering | ⚠️ only if listed **above** the launch entry; `postinstall`-entry ordering nuances |
| Synchronous (AUMID set before app starts) | ✅ `ewWaitUntilTerminated` | ✅ if **no** `nowait` |
| Checkable `ResultCode` | ✅ full control, can log/non-fatal | ❌ no per-step branching |
| Doesn't disturb the launch entry | ✅ launch `[Run]` entry untouched | ⚠️ must reorder `[Run]` |
| Always runs under `/VERYSILENT` | ✅ `ssPostInstall` always fires | ⚠️ must **avoid** `skipifsilent` |

**Verbatim `[Code]` to add to `QMKonnect.iss`** (add a new `CurStepChanged`
procedure alongside the existing `KillRunningInstance`/`InitializeSetup`/
`InitializeUninstall` — Inno recognizes event functions by name, no registration
needed):

```pascal
// Set the App User Model ID (AUMID) on the Start Menu shortcut so Windows toast
// notifications brand as "QMKonnect" (via set_aumid.ps1). ssPostInstall runs
// AFTER [Icons] created the .lnk and BEFORE the [Run] section launches the app,
// so the AUMID is present at first launch. ewWaitUntilTerminated makes it
// synchronous (no race with app startup). Failure is NON-FATAL: AUMID only
// affects notification branding, so we log and continue - never abort the install.
procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  LnkPath: String;
  PsArgs: String;
begin
  if CurStep <> ssPostInstall then
    Exit;

  // {#MyAppName} is the compile-time preprocessor -> "QMKonnect", matching the
  // [Icons] Name exactly; {userprograms} is the runtime Start Menu\Programs dir.
  LnkPath := ExpandConstant('{userprograms}\{#MyAppName}.lnk');
  if not FileExists(LnkPath) then
  begin
    Log('CurStepChanged: shortcut not found (' + LnkPath + '); skipping AUMID');
    Exit;
  end;

  // Embedded double-quotes wrap paths that may contain spaces. {tmp} already
  // holds set_aumid.ps1 (extracted in the [Files] phase).
  PsArgs := '-NoProfile -ExecutionPolicy Bypass -File "' +
    ExpandConstant('{tmp}\set_aumid.ps1') + '" "' + LnkPath + '" "Mulletware.QMKonnect"';

  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
       PsArgs, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('CurStepChanged: failed to launch set_aumid.ps1 (ResultCode=' + IntToStr(ResultCode) + ')')
  else if ResultCode <> 0 then
    Log('CurStepChanged: set_aumid.ps1 exited ' + IntToStr(ResultCode) + ' (non-fatal)');
end;
```

**Why these specifics:**

- **`{sys}\WindowsPowerShell\v1.0\powershell.exe`** — the canonical, always-present
  location on Windows; avoids any `PATH`/app-execution-alias ambiguity that a bare
  `powershell.exe` can hit. (`{sys}` = `System32`.)
- **`SW_HIDE`** hides the PowerShell window, matching the existing
  `KillRunningInstance` style (`SW_HIDE, ewWaitUntilTerminated, ResultCode`).
  *(Note: the task brief mentioned `SW_HIDE, ew_HIDE, ewWaitUntilTerminated` —
  `ew_HIDE` is not a valid `TExecWait` value; the valid waits are `ewNoWait`,
  `ewWaitUntilTerminated`, `ewWaitUntilIdle`. Use `SW_HIDE` for the show-cmd.)*
- **`ewWaitUntilTerminated`** makes it synchronous: the install does not proceed
  to the `[Run]` section (app launch) until `set_aumid.ps1` returns.
- **Non-fatal handling** — `Exec` returns `False` only if the process could not be
  *started*; `ResultCode` holds the exit code. We log both cases and never
  `Result := False` / abort, because a missing AUMID only degrades notification
  branding, not app function.
- **`FileExists` guard** — defensively confirms the `[Icons]` `.lnk` exists before
  invoking; avoids passing a dangling path to the script.

**Why `nowait` is NOT acceptable for this step:** `nowait` would launch PowerShell
and immediately proceed to the app-launch `[Run]` entry, **racing** the AUMID-set
against app startup. The app could read the `.lnk` before the AUMID is written.
We must wait — hence `ewWaitUntilTerminated` (or, if `[Run]` were used, **no**
`nowait`).

### 4. `[Run]` ordering interaction — top-to-bottom (moot under the recommended approach)

`[Run]` entries execute **top-to-bottom in listed order** (confirmed by the `[Run]`
docs: entries are processed in the order they appear in the script). So **if** a
`[Run]`-based approach were chosen, the set-AUMID entry would have to be listed
**above** the launch entry:

```ini
[Run]
; (Alternative to CurStepChanged - NOT recommended. Shown for completeness.)
; runhidden + NO nowait => synchronous + hidden. NO skipifsilent => always runs.
Filename: "{sys}\WindowsPowerShell\v1.0\powershell.exe"; Parameters: "-NoProfile -ExecutionPolicy Bypass -File ""{tmp}\set_aumid.ps1"" ""{userprograms}\{#MyAppName}.lnk"" ""Mulletware.QMKonnect"""; Flags: runhidden; StatusMsg: "Configuring notifications..."

; Existing launch entry - keep exactly as-is. Listed AFTER, so it runs second.
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent
```

Under the **recommended** `CurStepChanged` approach, the existing `[Run]` section
is **untouched** (it keeps its single launch entry), because `ssPostInstall`
(step 6) always precedes `[Run]` (step 7) regardless of `[Run]` internal ordering.

**Reference:** `[Run]` section. `https://jrsoftware.org/ishelp/index.php?topic=runsection`
(root → "Setup" → "[Run] section").

### 5. Uninstall — automatic, no extra code needed

✅ **Confirmed.** Inno automatically removes everything it created during a normal
uninstall, including shortcuts created via `[Icons]` (the default is to uninstall
the shortcut; `uninsneveruninstall` would be required to *keep* it, and this
`.iss` does not use it). The `[Registry]` HKCU `Run` value is likewise removed
because the entry carries `Flags: uninsdeletevalue` (verified in `QMKonnect.iss`).

Because the AUMID is stored **on the `.lnk` file** (a property set on the shortcut
itself), deleting the `.lnk` removes the AUMID too. **No `[UninstallRun]` and no
`CurUninstallStepChanged` work is needed.**

**Reference:** `[Icons]` section (default uninstall behavior).
`https://jrsoftware.org/ishelp/index.php?topic=iconssection`
(root → "Setup" → "[Icons] section").

### 6. Silent install (`/VERYSILENT`) — runs unless `skipifsilent`

✅ **Confirmed.** A `[Run]` entry runs during a silent install **unless** it carries
the `skipifsilent` flag. Therefore:

- **Set-AUMID step:** must **NOT** have `skipifsilent` — it must run even in
  headless/CI `/VERYSILENT` installs so the AUMID is always set. ✅
- **App-launch entry:** keeps `skipifsilent` (verified present in
  `QMKonnect.iss`) — a GUI tray app must not be spawned during headless
  verification runs. ✅

**Under the recommended `CurStepChanged` approach this is automatic:** `ssPostInstall`
fires **unconditionally** for all install modes (interactive, `/SILENT`,
`/VERYSILENT`) — it has no `skipifsilent`-equivalent. So the AUMID step always
runs, and the launch entry retains `skipifsilent` untouched.

**Reference:** `[Run]` section → `skipifsilent` / `skipifnotsilent` flags.
`https://jrsoftware.org/ishelp/index.php?topic=runsection`.

---

## Recommendation (consolidated)

Use **`[Files]` + `[Code] CurStepChanged(ssPostInstall)`**, and leave `[Run]`
untouched.

**1. `[Files]` — add one line:**
```ini
Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall
```
(`Source` is relative to the `.iss` dir; the file lives at
`packaging/windows/inno/set_aumid.ps1`.)

**2. `[Code]` — add `CurStepChanged`** (verbatim block in §3 above). Key points:
- Fires after `[Icons]`, before `[Run]` → AUMID set before first launch.
- `ewWaitUntilTerminated` + `SW_HIDE` → synchronous, hidden.
- Checkable `ResultCode`; non-fatal on failure (log only).
- Runs under all install modes (no `skipifsilent` concern).

**3. `[Run]` — no change.** The existing app-launch entry stays last and keeps
`nowait postinstall skipifsilent`.

**4. Uninstall — no change.** Inno auto-deletes the `[Icons]` `.lnk` (and its
AUMID) plus the `insdeletevalue` Run-key value.

---

## Reference URLs (canonical; confirm slugs against the live TOC)

- **Help root:** `https://jrsoftware.org/ishelp/index.php`
- **Pascal Scripting — `CurStepChanged` / `TSetupStep` (install-order facts):**
  `https://jrsoftware.org/ishelp/index.php?topic=scripting`
- **`[Run]` section (ordering, `nowait`, `skipifsilent`):**
  `https://jrsoftware.org/ishelp/index.php?topic=runsection`
- **`[Files]` section / `deleteafterinstall` flag:**
  `https://jrsoftware.org/ishelp/index.php?topic=filescommon`
- **`[Icons]` section (auto-uninstall of shortcuts):**
  `https://jrsoftware.org/ishelp/index.php?topic=iconssection`
- **`Exec` / support functions (`FileExists`, `Log`, `IntToStr`):**
  `https://jrsoftware.org/ishelp/index.php?topic=scripting`

> Confidence: the **factual claims** (order, flags, behavior) are high-confidence
> stable Inno mechanics. The exact `topic=` URL **slugs** are reconstructed from
> memory and should be clicked through the live TOC to confirm before citing in
> code comments.

---

## Gaps / verification steps

- **`set_aumid.ps1` does not yet exist** in the repo (not present at
  `packaging/windows/inno/`); it must be authored (the AUMID-setting logic via
  `WScript.Shell` COM is out of scope for this research task — confirm the script
  accepts `(lnkPath, aumid)` positional args as the `[Code]` call assumes).
- **Live-doc confirmation** of the four `topic=` URL slugs above (couldn't fetch
  in-sandbox). The root + topic titles are correct; only the slugs need a click.
- **`postinstall` + non-`postinstall` `[Run]` ordering** is a known subtle area —
  one of several reasons the `CurStepChanged` approach (which sidesteps `[Run]`
  entirely) is recommended over a `[Run]` entry.

---

## Supervisor coordination
None needed — task fully answerable from Inno-Setup knowledge + the repo's actual
`.iss`. Returning the completed brief.