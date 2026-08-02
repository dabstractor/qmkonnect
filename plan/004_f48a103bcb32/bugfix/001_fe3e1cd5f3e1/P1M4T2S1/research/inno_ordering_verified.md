# Inno Setup post-install integration — VERIFIED ordering (supersedes subagent note)

> **CORRECTION.** The sibling file `inno_postinstall_patterns.md` (written by a
> sandboxed subagent without web access) claims `CurStepChanged(ssPostInstall)`
> runs **before** the `[Run]` section. **That is backwards.** The authoritative
> StackOverflow answer for the exact question *"Is the [Run] section processed
> before the CurStepChanged event for the ssPostInstall step?"* states:
>
> > "That is true. The [Run] section entries are processed **before** the
> > CurStepChanged event for the ssPostInstall step is fired."
> > — https://stackoverflow.com/questions/29841276
>
> So the real order is: `[Files] → [Icons] → [INI] → [Registry] → **[Run]** → **CurStepChanged(ssPostInstall)** → [Done]`.
> (Source: jrsoftware.org/ishelp `topic_installorder.htm`, `topic_scriptevents.htm`,
> `topic_runsection.htm`.)

## Why this does NOT change the chosen approach

The chosen integration is `[Code] procedure CurStepChanged(ssPostInstall)`. Its
correctness depends on exactly ONE ordering fact:

> **`ssPostInstall` runs AFTER `[Icons]`.** ✅ — unambiguous and version-stable.

`[Icons]` is part of "the actual installation"; `ssPostInstall` fires "just after
the actual installation finishes." So the Start Menu `.lnk` **always exists** by
the time `CurStepChanged(ssPostInstall)` runs — the only guarantee we need. The
`[Run]`-vs-`ssPostInstall` relationship is **irrelevant** to whether the `.lnk`
exists (it does) and the AUMID gets set (it will).

### The app-launch timing (harmless)

Because `[Run]` precedes `ssPostInstall`, the app-launch `[Run]` entry (which is
`nowait`) may have already started the app before `set_aumid.ps1` runs. **This is
harmless:**

- A toast fires ONLY on a `rules.toml` parse error inside `host_context_for_window`
  (`src/core/notifier.rs`), which triggers on a **window focus change**.
- At install time the installer has the foreground; the app cannot receive a
  focus-change event, parse `rules.toml`, and fire a toast in the sub-second
  window between app launch and `ssPostInstall`'s `set_aumid.ps1` completing.
- By the time the user actually switches windows (post-install, post-wizard), the
  `.lnk` long since carries the AUMID.

So the toast renders correctly on the FIRST real trigger. No race in practice.

## Chosen approach: `[Files]` (bundle to `{tmp}`) + `[Code] CurStepChanged(ssPostInstall)` + `Exec`

Why `[Code]` (not a `[Run]` entry):

1. **Matches the existing `.iss` idiom verbatim.** `KillRunningInstance` already
   uses `Exec(ExpandConstant('{cmd}'), '/C …', '', SW_HIDE, ewWaitUntilTerminated,
   ResultCode)`. The new `CurStepChanged` copies that exact shape → lowest
   cognitive load, copy-paste-ready.
2. **Non-fatal + logged.** `Exec` returns a `ResultCode`; we `Log()` non-zero and
   continue (AUMID is branding-only). A `[Run]` entry, by contrast, raises an
   Inno error dialog on a non-zero exit code unless the script is careful.
3. **Does not disturb the `[Run]` section.** No reordering, no flag analysis.
4. **Unconditional across modes.** `ssPostInstall` fires for interactive,
   `/SILENT`, and `/VERYSILENT` installs alike (no `skipifsilent` analogue). The
   AUMID is always set, even in headless/CI installs.
5. **Synchronous within itself.** `ewWaitUntilTerminated` ensures the AUMID write
   completes before `ssPostInstall` returns (no `nowait` race).

### The `[Files]` entry (bundle the helper, do NOT leave it in `{app}`)

```ini
Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall
```

- `Source` is **relative to the `.iss` dir** → resolves to
  `packaging/windows/inno/set_aumid.ps1` (same dir as `QMKonnect.iss`; consistent
  with the existing `{#AssetDir}` / `{#ReleaseDir}` relative entries).
- `DestDir: "{tmp}"` → reachable at runtime via
  `ExpandConstant('{tmp}\set_aumid.ps1')`; never installed under `{app}`.
- `deleteafterinstall` → removed after install (incl. on failure). Clean.
- `[Files]` extraction (step 2) happens well before `ssPostInstall` (step 7), so
  the script is present when invoked. ✅
- **`DestDir` is MANDATORY** for every `[Files]` entry (compiler errors
  *"Required parameter DestinationDir missing"* if omitted).

### The `[Code]` procedure (verbatim — to add to QMKonnect.iss `[Code]`)

```pascal
// (P1.M4.T2.S1) Set the App User Model ID (AUMID) on the Start Menu shortcut so
// WinRT toasts render as "QMKonnect". ssPostInstall runs AFTER [Icons] created
// the .lnk (the only ordering fact we need) — the [Run] app-launch may have
// already fired, but that's harmless: toasts only trigger on a post-startup
// window-focus rules.toml parse error, never during install. Failure is NON-FATAL
// (AUMID affects only notification branding): log and continue, never abort.
procedure CurStepChanged(CurStep: TSetupStep);
var
  ResultCode: Integer;
  LnkPath: String;
  PsArgs: String;
begin
  if CurStep <> ssPostInstall then
    Exit;
  LnkPath := ExpandConstant('{userprograms}\{#MyAppName}.lnk');
  if not FileExists(LnkPath) then begin
    Log('CurStepChanged: Start Menu shortcut not found (' + LnkPath + '); skipping AUMID');
    Exit;
  end;
  PsArgs := '-NoProfile -ExecutionPolicy Bypass -File "' +
            ExpandConstant('{tmp}\set_aumid.ps1') + '" "' + LnkPath +
            '" "Mulletware.QMKonnect"';
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
       PsArgs, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('CurStepChanged: set_aumid.ps1 could not start (ResultCode=' +
        IntToStr(ResultCode) + ') — non-fatal')
  else if ResultCode <> 0 then
    Log('CurStepChanged: set_aumid.ps1 exited ' + IntToStr(ResultCode) + ' — non-fatal');
end;
```

Notes:
- `{sys}\WindowsPowerShell\v1.0\powershell.exe` — canonical, always-present on
  Windows 10/11; avoids `PATH` / app-execution-alias ambiguity of bare `powershell`.
- `SW_HIDE` hides the console (matches `KillRunningInstance`).
- `ewWaitUntilTerminated` (valid `TExecWait` values: `ewNoWait`,
  `ewWaitUntilTerminated`, `ewWaitUntilIdle`). **Not** `ew_HIDE` (invalid).
- Inno recognizes event functions by NAME (`CurStepChanged`) — no registration.

## Uninstall — automatic, no extra code

Inno auto-removes everything it created, including `[Icons]` shortcuts (the
default; `uninsneveruninstall` would be needed to keep one, and this `.iss`
doesn't use it). The `[Registry]` Run value has `Flags: uninsdeletevalue`. Since
the AUMID is a property **on the `.lnk` file**, deleting the `.lnk` removes the
AUMID. **No `[UninstallRun]` / `CurUninstallStepChanged` needed.**

## Silent install (`/VERYSILENT`)

`ssPostInstall` fires unconditionally → AUMID step always runs. The existing
app-launch `[Run]` entry keeps `skipifsilent` (correct — don't spawn a tray app
in a headless/CI run). ✅

## Reference URLs

- Help root: https://jrsoftware.org/ishelp/index.php
- Install order: https://jrsoftware.org/ishelp/topic_installorder.htm
- Event functions (CurStepChanged / TSetupStep): https://jrsoftware.org/ishelp/topic_scriptevents.htm
- `[Run]` section: https://jrsoftware.org/ishelp/topic_runsection.htm
- `[Files]` section / `deleteafterinstall`: https://jrsoftware.org/ishelp/topic_filescommon.htm
- `[Icons]` section (auto-uninstall): https://jrsoftware.org/ishelp/topic_iconssection.htm
- [Run] vs ssPostInstall ordering (Q&A): https://stackoverflow.com/questions/29841276