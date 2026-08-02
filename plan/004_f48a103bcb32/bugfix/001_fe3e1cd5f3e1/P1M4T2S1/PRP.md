# PRP — P1.M4.T2.S1: Start Menu shortcut AUMID (Inno installer) for toast rendering

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files (4, all Windows-packaging — NO Rust):**
> `packaging/windows/inno/set_aumid.ps1` (NEW), `packaging/windows/inno/QMKonnect.iss`
> (EDIT), `packaging/windows/install.ps1` (EDIT, parity), `packaging/windows/inno/README.md` (EDIT).
>
> **Approach:** the Inno installer owns the Start Menu `.lnk`; we set the AUMID on
> it post-install via a PowerShell + `Add-Type` (C# P/Invoke) helper. This is the
> **only** viable path — see "Approach decision" below. P1.M4.T1 chose **Approach A**
> (raw `windows` crate, NOT `tauri-winrt-notification`), so the app does **not**
> create the shortcut at runtime → approach C (runtime fallback) does **not** apply.
>
> **This closes the final link in the toast chain.** P1.M4.T1.S1 set the *process*
> AUMID (`SetCurrentProcessExplicitAppUserModelID("Mulletware.QMKonnect")` at
> startup) + enabled the WinRT features. P1.M4.T1.S2 implemented the actual toast
> call (`ToastNotificationManager::CreateToastNotifierWithId(&APP_AUMID)` → `Show`).
> But a WinRT toast is **silently suppressed** unless a Start Menu `.lnk` advertises
> the SAME AUMID via the `System.AppUserModel.ID` property (research §Q7 of the S2
> PRP; Microsoft toast docs). **This task sets that property on the `.lnk`** the
> installer already creates — the last prerequisite for toasts to render.

---

## Goal

**Feature Goal**: After the Inno installer (and the `install.ps1` dev-loop
installer) runs, the Start Menu shortcut at
`%APPDATA%\Microsoft\Windows\Start Menu\Programs\QMKonnect.lnk` carries the
property `System.AppUserModel.ID = "Mulletware.QMKonnect"` — the exact string the
running process advertises (`src/platforms/mod.rs::APP_AUMID`), so WinRT toasts
render instead of being silently dropped.

**Deliverable** (exactly four files — all Windows packaging, zero Rust):
1. **`packaging/windows/inno/set_aumid.ps1`** (NEW) — a self-contained PowerShell
   helper that takes `<lnk-path> <aumid>`, `Add-Type`s a C# class that does the
   `CoCreateInstance(CLSID_ShellLink)` → `IPersistFile.Load` → `IPropertyStore.SetValue(PKEY_AppUserModel_ID, VT_LPWSTR)` → `Commit` → `IPersistFile.Save`
   dance, and **always exits 0** (non-fatal). Includes a `Get` verifier.
2. **`packaging/windows/inno/QMKonnect.iss`** (EDIT) — `[Files]` bundles the helper
   to `{tmp}` (`deleteafterinstall`); `[Code]` adds a `CurStepChanged(ssPostInstall)`
   that runs the helper against the Start Menu `.lnk` with the literal
   `"Mulletware.QMKonnect"`. Non-fatal + logged.
3. **`packaging/windows/install.ps1`** (EDIT) — parity: after it creates the `.lnk`
   via `WScript.Shell`, call the shared helper so dev-loop installs render toasts
   too (the `.iss` header states it "Replicates ../install.ps1 exactly").
4. **`packaging/windows/inno/README.md`** (EDIT, Mode A) — note the AUMID-shortcut
   requirement for toast notifications + a verification command.

**Success Definition**:
- On the **Linux dev box**: this PRP touches ONLY `packaging/` + `docs` — **zero
  Rust files**. `cargo build` / `cargo test --bin qmkonnect -- --test-threads=1`
  are unchanged-green regression baselines (proof no Rust was touched). The `.iss`
  / `.ps1` are not compiled on Linux; they are reviewed for correctness.
- On **Windows** (DEFERRED to the AGENTS.md Windows dev loop — the implementing
  agent is on Linux): `build.ps1` compiles the installer; running it installs a
  Start Menu `.lnk` whose AUMID verifies as `Mulletware.QMKonnect`; with
  P1.M4.T1.S1+S2 landed, a deliberately-broken `rules.toml` triggers a real toast
  (auto-dismissing, Action Center) instead of a modal or silence.

## User Persona (if applicable)

**Target User**: the Windows end user whose `rules.toml` fails to parse. Before
this task (with S1+S2 landed but not T2.S1), `Show()` returns `Ok(())` and
**nothing appears** — the toast is silently suppressed because no `.lnk`
advertises the AUMID. After T2.S1, the toast renders.
**Use Case**: user edits `rules.toml`, introduces a syntax error, switches
window focus → `host_context_for_window` re-parses, fails, dedup fires once → a
toast slides in ("QMKonnect: rules.toml invalid" + the parse error), auto-dismisses,
lands in Action Center.
**Pain Points Addressed**: the silent-suppression gap (S1+S2 work but produce no
visible output without this task). Completes the S1→S2→**T2.S1** toast chain.

## Why

- **A WinRT toast is silently dropped without a matching `.lnk` AUMID.** Windows'
  toast subsystem keys notifications to the originating app's AUMID and requires a
  Start Menu shortcut advertising that AUMID before it will render. S1 set the
  process AUMID; S2 emits the toast; **T2.S1 advertises the AUMID on the `.lnk`**
  — without it the whole chain is invisible. (Microsoft "Send a local toast from
  desktop" quickstart; S2 PRP research §Q7.)
- **The `.lnk` already exists — it just lacks the AUMID property.** Inno's `[Icons]`
  section (`QMKonnect.iss:93`) creates `QMKonnect.lnk` pointing at the exe, but
  `System.AppUserModel.ID` is **not** a standard shortcut field. Setting it requires
  `IPropertyStore::SetValue`, which Inno's Pascal `CreateOleObject('WScript.Shell')`
  (IDispatch automation) **cannot reach** — it needs vtable COM. Hence the helper.
- **`install.ps1` has the same gap.** It creates the `.lnk` via `WScript.Shell`
  (`install.ps1:60-66`) with no AUMID. The `.iss` header explicitly says it
  "Replicates ../install.ps1 exactly" — both installers must set the AUMID or the
  invariant is broken and dev-loop installs silently produce no toast.
- **Single source of truth.** The AUMID string `"Mulletware.QMKonnect"` is
  defined once in `src/platforms/mod.rs:134` (`pub const APP_AUMID`). The `.iss`
  and `.ps1` hardcode the same literal (Inno/PS can't read the Rust const) — they
  MUST match, or toasts are silently dropped. This PRP pins the literal in all
  three places and documents the cross-reference.

## What

### Approach decision: Inno-installer post-install helper (NOT runtime fallback)

The contract offered three paths. **Two are eliminated; one remains:**

- **(A) Inno `[Code]` direct COM `IShellLinkW` + `IPropertyStore`** — **impossible.**
  Inno's Pascal Script COM support is **IDispatch-only** (`CreateOleObject`).
  `IPropertyStore` is a vtable interface, not IDispatch, so it cannot be called
  from Pascal Script directly. Eliminated.
- **(B) post-install PowerShell + `Add-Type` (C# P/Invoke)** — **CHOSEN.** A
  small helper script bundles the verified COM recipe. This is the documented
  Microsoft pattern (desktop-toast quickstart) and is robust/testable.
- **(C) runtime shortcut creation by the app** — **does not apply.** P1.M4.T1 chose
  **Approach A** (raw `windows` crate) specifically, **rejecting**
  `tauri-winrt-notification` (which offers `PowerShell::create_shortcut`). The app
  therefore does NOT create the `.lnk` at runtime; the Inno installer owns it.
  (S1 PRP "Approach selection: A … not B" + findings.md rationale #2: "the plan's
  task split implies the Inno installer owns the shortcut.")

→ **Implement approach B**: a `set_aumid.ps1` helper invoked from Inno `[Code]
CurStepChanged(ssPostInstall)`, and (parity) from `install.ps1`.

### Execution-order note (read carefully — the prior research got this backwards)

A sandboxed subagent's note (`research/inno_postinstall_patterns.md`) claimed
`ssPostInstall` runs **before** `[Run]`. **That is wrong.** The authoritative
answer (https://stackoverflow.com/q/29841276, citing Inno's install order) is:
`[Files] → [Icons] → [INI] → [Registry] → **[Run]** → **CurStepChanged(ssPostInstall)**`.

**This does not affect correctness.** Our `CurStepChanged(ssPostInstall)` depends
only on: *"runs after `[Icons]`"* — which is unambiguous (ssPostInstall = "just
after the actual installation finishes"; `[Icons]` is part of installation). So
the `.lnk` **always exists** when the helper runs. The app-launch `[Run]` entry
(`nowait`) may have already started the app — **harmless**: a toast fires only on
a `rules.toml` parse error inside `host_context_for_window` on a **window focus
change**, which cannot occur during the installer's final wizard step (the
installer holds the foreground). By the time the user actually switches windows
post-install, the `.lnk` long since carries the AUMID. Verified in
`research/inno_ordering_verified.md`.

### Success Criteria
- [ ] `packaging/windows/inno/set_aumid.ps1` exists; `param($LnkPath,$Aumid)`;
      `Add-Type`s `QMKonnect.ShortcutAumid` with a re-run guard
      (`-as [type]`); calls `::Set($LnkPath,$Aumid)`; **always `exit 0`**; includes
      a `Get()` for verification.
- [ ] `set_aumid.ps1`'s C# uses `[StructLayout(LayoutKind.Explicit)]` PROPVARIANT
      (`vt`@0, `pwszVal`@8, `pad`@16 → 24 bytes on x64), `VT_LPWSTR`=31, the exact
      GUIDs, and calls **both** `IPropertyStore.Commit()` **and** `IPersistFile.Save()`.
- [ ] `QMKonnect.iss` `[Files]` has `Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall`.
- [ ] `QMKonnect.iss` `[Code]` has `procedure CurStepChanged(CurStep: TSetupStep)`
      that, at `ssPostInstall`, `Exec`s `powershell.exe -NoProfile -ExecutionPolicy
      Bypass -File "{tmp}\set_aumid.ps1" "{userprograms}\QMKonnect.lnk" "Mulletware.QMKonnect"`
      with `SW_HIDE, ewWaitUntilTerminated`, non-fatal `Log` on failure.
- [ ] `install.ps1` calls the same helper against its `$StartMenu` `.lnk`
      immediately after `$s.Save()` (so dev-loop installs render toasts).
- [ ] `packaging/windows/inno/README.md` documents the AUMID-shortcut requirement
      and the verification command.
- [ ] **Zero Rust files modified** (`git diff --stat` shows only the 4 packaging/
      docs files). `cargo build` + `cargo test --bin qmkonnect -- --test-threads=1`
      are unchanged-green.
- [ ] The AUMID literal is **identical** in three places: `src/platforms/mod.rs:134`
      (`APP_AUMID`, read-only ref), `QMKonnect.iss` `[Code]`, `install.ps1` → all
      `"Mulletware.QMKonnect"`.

## All Needed Context

### Context Completeness Check
_Pass._ An agent with no prior knowledge can build all four artifacts from: the
verified C#/COM recipe + exact GUIDs (`research/aumid_recipe.md`), the corrected
Inno ordering + verbatim `[Code]`/`[Files]` snippets
(`research/inno_ordering_verified.md`), the exact current text of the `.iss`
sections (quoted in Tasks), the exact `install.ps1` shortcut block (quoted in
Task 3), and the README anchors (Task 4). The two non-trivial technical pieces
(PROPVARIANT layout; Commit-doesn't-persist) are spelled out with the failure
mode. The validation asymmetry (Linux agent can't run ISCC/PowerShell) is a
documented deferred gate; the Linux gates prove "no Rust touched."

### Documentation & References

```yaml
# MUST READ — the verified C#/PowerShell recipe + GUIDs + the 4 critical gotchas
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T2S1/research/aumid_recipe.md
  why: "the EXACT COM call sequence, the verbatim GUIDs (CLSID_ShellLink {00021401-…},
        IID_IPersistFile {0000010B-…}, IID_IPropertyStore {886D8EEB-…},
        PKEY_AppUserModel_ID {9F4C2855-…} pid 5), the PROPVARIANT LayoutKind.Explicit
        layout (vt@0, pwszVal@8, pad@16 = 24 bytes on x64, VT_LPWSTR=31), and the
        #1/#2 gotchas (layout wrong = silent fail; Commit alone does NOT persist →
        need IPersistFile.Save). This IS the set_aumid.ps1 implementation."
  section: "all; the C# block + '4 most critical gotchas'"
  critical: "PROPVARIANT MUST be LayoutKind.Explicit with the exact FieldOffsets, or SetValue
        silently fails or AVs. Commit() flushes in-memory only — IPersistFile.Save() is REQUIRED.
        No CoInitializeEx from PowerShell (PS5.1 is STA, auto-inited). Add-Type needs a re-run
        guard (`-as [type]`) or it throws on the 2nd call."

# MUST READ — the CORRECTED Inno ordering + verbatim [Files]/[Code] snippets
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T2S1/research/inno_ordering_verified.md
  why: "CORRECTS the sibling inno_postinstall_patterns.md which got [Run]↔ssPostInstall
        backwards. Establishes: ssPostInstall runs AFTER [Icons] (the only fact we need →
        the .lnk always exists). Gives the verbatim [Files] deleteafterinstall line and the
        verbatim CurStepChanged Pascal procedure. Explains why post-launch timing is harmless.
        Uninstall is automatic (Inno deletes [Icons] .lnk); /VERYSILENT runs the step unconditionally."
  section: "all"
  critical: "use [Code] CurStepChanged(ssPostInstall) + Exec — it matches the existing
        KillRunningInstance Exec idiom, is non-fatal+logged, and runs unconditionally. {sys}\WindowsPowerShell\v1.0\powershell.exe
        (canonical, avoids PATH/alias issues). SW_HIDE (matches KillRunningInstance). DestDir: '{tmp}' is MANDATORY on [Files]."

# MUST READ — the S2 PRP (the toast call this enables); confirms Show() returns Ok without the .lnk
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S2/PRP.md
  why: "documents that ToastNotificationManager::CreateToastNotifierWithId(&APP_AUMID) → Show
        returns Ok(()) but the toast is SILENTLY SUPPRESSED until a Start Menu .lnk advertises
        APP_AUMID (research §Q7). T2.S1 is exactly that .lnk. Confirms APP_AUMID = 'Mulletware.QMKonnect'."
  section: "Goal, Known Gotchas (toast won't render until T2.S1)"
  critical: "do NOT expect a visible toast from S1+S2 alone. T2.S1 (this task) is the final link.
        The .lnk's AUMID MUST equal APP_AUMID byte-for-byte or the toast is dropped."

# MUST READ — the S1 PRP (defines APP_AUMID, the process AUMID; the source of truth)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S1/PRP.md
  why: "APP_AUMID = 'Mulletware.QMKonnect' (NOT the Inno AppId GUID {FAAE1F7A-…}). set_aumid()
        sets the PROCESS AUMID at startup; T2.S1 sets the SHORTCUT AUMID. Both must match."
  section: "Goal, Known Gotchas (AUMID ≠ Inno AppId)"
  critical: "the AUMID literal is the single source of truth. The .iss/.ps1 hardcode it as a
        string literal (Inno/PS cannot read the Rust const) and MUST match src/platforms/mod.rs:134."

# MUST READ — the .iss being edited (exact [Files]/[Icons]/[Code] text in Tasks 2)
- file: packaging/windows/inno/QMKonnect.iss
  why: "the [Files] section (lines 84-90), [Icons] line 93, [Code] section (line 107+, with
        KillRunningInstance Exec idiom at :112). Task 2 adds one [Files] line and one [Code]
        procedure. MyAppName='QMKonnect' (line 23) → {userprograms}\\{#MyAppName}.lnk."
  pattern: "the existing KillRunningInstance uses `Exec(ExpandConstant('{cmd}'), '/C …', '',
        SW_HIDE, ewWaitUntilTerminated, ResultCode)` — the CurStepChanged Exec mirrors this exactly.
        Inno event functions are recognized by NAME (no registration)."
  gotcha: "do NOT touch [Icons] (the .lnk is created there as-is — correct), [Run] (the app-launch
        entry stays last with nowait postinstall skipifsilent), or [Setup]/[Registry]. DestDir on
        [Files] is MANDATORY (compiler errors without it)."

# MUST READ — install.ps1 being edited (exact shortcut block in Task 3)
- file: packaging/windows/install.ps1
  why: "lines 60-66 create the .lnk via `$Wsh.CreateShortcut($StartMenu) … $s.Save()`. Task 3 adds
        a call to the shared helper right after Save(). `$PSScriptRoot` = packaging/windows/, so the
        helper is at `Join-Path $PSScriptRoot 'inno\\set_aumid.ps1'`. $StartMenu is the .lnk path."
  pattern: "install.ps1 already calls external scripts (it copies uninstall.ps1); calling the helper
        matches that idiom. $ErrorActionPreference='Stop' is set at top — wrap the helper call so a
        failure never aborts the install (AUMID is branding-only)."
  gotcha: "the AUMID literal here MUST equal the .iss literal AND APP_AUMID. Use a $Aumid variable
        set once. Don't let a helper failure abort install.ps1 (try/catch + continue)."

# REFERENCE — the AUMID const (single source of truth; read-only)
- file: src/platforms/mod.rs
  why: "line 134: pub const APP_AUMID: &str = 'Mulletware.QMKonnect'. This is the authoritative
        AUMID. The .iss/.ps1 hardcode the same literal. DO NOT edit this file in T2.S1 (read-only)."
  pattern: "the toast notifier (S2, mod.rs:239) uses HSTRING::from(APP_AUMID); the process setter
        (S1, mod.rs:149 set_aumid) uses the same const. The .lnk must advertise the same string."
  gotcha: "read-only. Do NOT modify any Rust file in this task (the deliverable is packaging-only)."

# EXTERNAL — Microsoft: why the .lnk AUMID is required for desktop toasts
- url: https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl
  why: "establishes that a desktop Win32 app sending toasts MUST have (a) a registered AUMID and
        (b) a Start Menu shortcut with System.AppUserModel.ID set to that AUMID — exactly this task.
        Also shows the IPropertyStore/IPersistFile sequence used by set_aumid.ps1."
  critical: "without the .lnk AUMID, Show() succeeds but nothing renders. This is why S1+S2 alone
        produce no visible toast — T2.S1 is required."

# EXTERNAL — IPropertyStore::SetValue (why Commit alone is not enough)
- url: https://learn.microsoft.com/en-us/windows/win32/api/propsys/nf-propsys-ipropertystore-setvalue
  why: "docs state 'SetValue affects the current property store instance only' — i.e. in-memory.
        Confirms that IPropertyStore.Commit flushes the store but IPersistFile.Save persists the .lnk."
  critical: "omitting IPersistFile.Save is the #2 failure: the AUMID looks set but is lost on release."

# EXTERNAL — the AUMID property key (PKEY) + VT type
- url: https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/properties/props-system-appusermodel-id.md
  why: "PKEY_AppUserModel_ID = {9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3} pid 5; PropVariant type VT_LPWSTR
        (31), .NET String. Confirms the property key + variant type set_aumid.ps1 uses."
  critical: "pid is 5 (not 0). fmtid/pid are the exact bytes in the PropertyKey struct."

# EXTERNAL — Inno install order + [Run]/ssPostInstall (the corrected ordering)
- url: https://stackoverflow.com/questions/29841276/is-the-run-section-processed-before-the-curstepchanged-event-for-the-sspostins
  why: "definitively answers '[Run] is processed BEFORE CurStepChanged(ssPostInstall)' — correcting
        the subagent note. Confirms ssPostInstall still runs AFTER [Icons] (what we rely on)."
  critical: "do NOT claim the helper runs before app launch — it doesn't (harmless, documented)."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
packaging/windows/inno/
  - QMKonnect.iss                  # EDIT: [Files] +1 line, [Code] +CurStepChanged
      :23  #define MyAppName "QMKonnect"
      :84-90 [Files] (3 Source lines for exe/Icon.ico/IconTray-dark.png)   # +1 Source line for set_aumid.ps1
      :92-93 [Icons] (Name: "{userprograms}\{#MyAppName}" … )              # DO NOT TOUCH (creates the .lnk correctly)
      :101-105 [Run] (single app-launch entry, nowait postinstall skipifsilent)  # DO NOT TOUCH
      :107-130 [Code] (KillRunningInstance / InitializeSetup / InitializeUninstall)  # +CurStepChanged
      :112  Exec(…, SW_HIDE, ewWaitUntilTerminated, ResultCode) idiom       # PRECEDENT to mirror
  - build.ps1                      # DO NOT TOUCH (reads version from Cargo.toml, runs iscc)
  - README.md                      # EDIT: +AUMID note + verification command
  - set_aumid.ps1                  # NEW (the AUMID-setting helper)
packaging/windows/install.ps1      # EDIT: call shared helper after $s.Save() (parity)
src/platforms/mod.rs:134           # READ-ONLY (pub const APP_AUMID = "Mulletware.QMKonnect")
```

### Desired Codebase tree with files added/changed

```bash
packaging/windows/inno/set_aumid.ps1    # NEW — Add-Type C# IPropertyStore helper; Set(lnk,aumid)+Get(lnk); always exit 0
packaging/windows/inno/QMKonnect.iss    # [Files] +1 (bundle set_aumid.ps1 to {tmp}, deleteafterinstall)
                                         # [Code] +CurStepChanged(ssPostInstall) → Exec powershell → set_aumid.ps1
packaging/windows/install.ps1           # +call shared helper after WScript.Shell Save() (dev-loop parity)
packaging/windows/inno/README.md        # +AUMID-shortcut requirement note + verification command
# (NO Rust files; NO Cargo.toml; NO new deps)
```

### Known Gotchas of our codebase & Library Quirks

```powershell
# CRITICAL (AUMID literal is the single source of truth): the .iss [Code] and install.ps1 BOTH
#   hardcode "Mulletware.QMKonnect" (Inno/PS can't read the Rust const). It MUST equal
#   src/platforms/mod.rs:134 APP_AUMID byte-for-byte, or toasts are silently dropped (the toast
#   notifier keys off APP_AUMID; the .lnk must advertise the SAME string). A mismatch = silent fail.

# CRITICAL (PROPVARIANT layout is the #1 failure cause): the C# struct MUST be
#   [StructLayout(LayoutKind.Explicit)] with vt at [FieldOffset(0)] (ushort), pwszVal at
#   [FieldOffset(8)] (IntPtr), pad at [FieldOffset(16)] (IntPtr) → 24 bytes on x64. LayoutKind.Sequential
#   produces wrong padding → SetValue silently fails or AVs. VT_LPWSTR = 31. See aumid_recipe.md.

# CRITICAL (Commit does NOT persist the .lnk): IPropertyStore::SetValue "affects the current property
#   store instance only" (MS docs). IPropertyStore.Commit flushes the in-memory store but does NOT
#   write the .lnk file. IPersistFile.Save(path, true) is REQUIRED. Omit it → AUMID set in memory,
#   lost on release. Always call Save after Commit.

# CRITICAL (no CoInitializeEx from PowerShell): PowerShell 5.1 runs STA and auto-initializes COM.
#   Calling CoInitializeEx manually risks RPC_E_CHANGED_MODE. The C# helper must NOT call it.
#   No elevation needed — the .lnk lives under %APPDATA% (user-writable); installer is per-user.

# CRITICAL (Add-Type re-run guard): Add-Type compiles into the AppDomain; a 2nd Add-Type for an
#   already-defined type THROWS. Guard: `if (-not ('QMKonnect.ShortcutAumid' -as [type])) { Add-Type … }`.
#   The installer invokes the script once, but install.ps1 / manual re-runs need the guard.

# CRITICAL (the helper must NEVER abort the install): set_aumid.ps1 ALWAYS `exit 0`. AUMID affects
#   only notification branding — a failure must never block the install. install.ps1 wraps the call
#   in try/catch; the .iss [Code] logs non-zero ResultCode and continues.

# CRITICAL (validation asymmetry — implementing agent runs on LINUX): ISCC (Inno compiler),
#   PowerShell, and Windows COM are unavailable on Linux. The .iss/.ps1 are NOT compiled/run there.
#   Linux gates: (1) `git diff --stat` proves NO Rust file touched; (2) cargo build/test are
#   unchanged-green regression baselines. The real gates (build.ps1 → install → verify AUMID →
#   toast renders) are DEFERRED to the AGENTS.md Windows dev loop.

# GOTCHA (Inno [Files] DestDir is MANDATORY): `Source: "set_aumid.ps1"` alone won't compile
#   ("Required parameter DestinationDir missing"). Use `DestDir: "{tmp}"; Flags: deleteafterinstall`.
#   Source path is RELATIVE TO THE .ISS DIR → resolves to packaging/windows/inno/set_aumid.ps1.

# GOTCHA (use {sys}\WindowsPowerShell\v1.0\powershell.exe, not bare powershell): the canonical
#   always-present path; avoids PATH / app-execution-alias ambiguity. Matches best practice.

# GOTCHA (do NOT touch [Icons], [Run], or [Setup]): the .lnk is created correctly in [Icons] as-is;
#   we only ANNOTATE it post-hoc. [Run]'s app-launch entry stays last (nowait postinstall skipifsilent).
#   Adding a [Run] entry instead of [Code] would raise an Inno error dialog on a non-zero exit code;
#   [Code] CurStepChanged gives clean non-fatal Log handling — use it.

# GOTCHA (uninstall is automatic): Inno removes [Icons] shortcuts on uninstall (default; no
#   uninsneveruninstall here). The AUMID lives ON the .lnk, so deleting the .lnk removes it. No
#   [UninstallRun] / CurUninstallStepChanged needed.

# GOTCHA (do NOT use the Inno AppId GUID as the AUMID): AppId {FAAE1F7A-…} is the installer upgrade
#   identity; the AUMID is the toast identity. They are different. Use "Mulletware.QMKonnect".

# GOTCHA (single-threaded cargo tests — AGENTS.md): if any Rust file were touched, cargo test needs
#   --test-threads=1. This task touches no Rust, but the regression gate keeps the flag for parity.
```

## Implementation Blueprint

### Data models and structure
None. No Rust types, no config, no CLI flags. One PowerShell helper script with a
C# interop class (`QMKonnect.ShortcutAumid::Set/Get`) and one Inno Pascal event
procedure (`CurStepChanged`). The only "data" is the AUMID string literal
`"Mulletware.QMKonnect"`, pinned identically in the `.iss`, `install.ps1`, and the
existing Rust const.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: CREATE packaging/windows/inno/set_aumid.ps1 (the verified C# IPropertyStore helper)
  Create a new file with EXACTLY this content (the C# is the production version — the
  recipe file's snippet had a couple of abbreviated tokens; this is correct & compileable):

  ```powershell
  <#
  .SYNOPSIS
      Sets System.AppUserModel.ID on an existing Start Menu .lnk (toast prerequisite).
  .DESCRIPTION
      Opens an existing shortcut via IPersistFile, sets its AUMID via IPropertyStore,
      commits, and persists via IPersistFile.Save. Used by the Inno installer
      (QMKonnect.iss CurStepChanged) and the dev-loop install.ps1 so WinRT toasts
      render as "QMKonnect" instead of being silently suppressed. The .lnk must already
      exist (created by the installer's [Icons] section / install.ps1's WScript.Shell).
      ALWAYS exits 0 — AUMID is notification-branding only; never abort the install.
  .PARAMETER LnkPath
      Absolute path to the .lnk (e.g. $env:APPDATA\...\Programs\QMKonnect.lnk).
  .PARAMETER Aumid
      The AUMID string. MUST equal src/platforms/mod.rs::APP_AUMID ("Mulletware.QMKonnect").
  #>
  param(
      [Parameter(Mandatory = $true, Position = 0)][string]$LnkPath,
      [Parameter(Mandatory = $true, Position = 1)][string]$Aumid
  )

  # Add-Type compiles into the AppDomain; guard against re-run (install.ps1 + manual runs).
  if (-not ('QMKonnect.ShortcutAumid' -as [type])) {
      Add-Type -TypeDefinition @'
  using System;
  using System.Runtime.InteropServices;

  namespace QMKonnect {
      [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),
       InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
      internal interface IPropertyStore {
          uint GetCount([Out] out uint cProps);
          uint GetAt([In] uint iProp, out PropertyKey pkey);
          uint GetValue([In] ref PropertyKey key, [Out] PropVariant pv);
          uint SetValue([In] ref PropertyKey key, [In] ref PropVariant pv);
          uint Commit();
      }

      [ComImport, Guid("0000010B-0000-0000-C000-000000000046"),
       InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
      internal interface IPersistFile {
          uint GetClassID([Out] out Guid pClassID);
          uint IsDirty();
          uint Load([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName, [In] uint dwMode);
          uint Save([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName, [In] bool fRemember);
          uint SaveCompleted([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
          uint GetCurFile([Out, MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
      }

      [StructLayout(LayoutKind.Sequential, Pack = 4)]
      internal struct PropertyKey {
          public Guid fmtid;
          public uint pid;
          public PropertyKey(Guid fmtid, uint pid) { this.fmtid = fmtid; this.pid = pid; }
      }

      // PROPVARIANT — LayoutKind.Explicit is MANDATORY (LayoutKind.Sequential = silent fail / AV).
      // VARTYPE at byte 0, three reserved WORDs (bytes 2..7), union pointer at byte 8.
      // 24 bytes on x64 (pad at byte 16). VT_LPWSTR = 31.
      [StructLayout(LayoutKind.Explicit)]
      internal struct PropVariant {
          [FieldOffset(0)]  public ushort vt;
          [FieldOffset(8)]  public IntPtr pwszVal;   // VT_LPWSTR pointer
          [FieldOffset(16)] public IntPtr pad;        // explicit 24-byte size

          public static PropVariant FromString(string s) {
              var pv = new PropVariant { vt = 31 };               // VT_LPWSTR
              pv.pwszVal = Marshal.StringToCoTaskMemUni(s);       // caller frees
              return pv;
          }
          public void Clear() {
              if (vt == 31 && pwszVal != IntPtr.Zero) Marshal.FreeCoTaskMem(pwszVal);
              vt = 0; pwszVal = IntPtr.Zero;
          }
      }

      internal static class Native {
          [DllImport("ole32.dll")]
          public static extern uint CoCreateInstance(
              [In] ref Guid rclsid, [In] IntPtr pUnkOuter, [In] uint dwClsContext,
              [In] ref Guid riid, [Out, MarshalAs(UnmanagedType.Interface)] out object ppv);
      }

      public static class ShortcutAumid {
          static readonly Guid CLSID_ShellLink  = new Guid("00021401-0000-0000-C000-000000000046");
          static readonly Guid IID_IPersistFile = new Guid("0000010B-0000-0000-C000-000000000046");
          static readonly PropertyKey PKEY_AppUserModel_ID =
              new PropertyKey(new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), 5);

          // Returns 0 on success, non-zero on failure (E_FAIL = 0x80004005).
          public static int Set(string lnkPath, string aumid) {
              object obj;
              Guid iid = IID_IPersistFile;
              uint hr = Native.CoCreateInstance(ref CLSID_ShellLink, IntPtr.Zero, 0x1, ref iid, out obj);
              if (hr != 0) return unchecked((int)hr);
              try {
                  var pf = (IPersistFile)obj;
                  if (pf.Load(lnkPath, 0x00000002) != 0) return unchecked((int)0x80004005); // STGM_READWRITE
                  // The ShellLink CoClass implements both interfaces — the cast triggers QI via the RCW.
                  var ps = (IPropertyStore)obj;
                  var pv = PropVariant.FromString(aumid);
                  try {
                      if (ps.SetValue(ref PKEY_AppUserModel_ID, ref pv) != 0)
                          return unchecked((int)0x80004005);
                      ps.Commit();
                  } finally { pv.Clear(); }
                  // Commit flushes in-memory only — persist the .lnk to disk.
                  if (pf.Save(lnkPath, true) != 0) return unchecked((int)0x80004005);
                  return 0;
              } finally { Marshal.ReleaseComObject(obj); }
          }

          // Read the AUMID back (for verification). Returns null if unset/unreadable.
          public static string Get(string lnkPath) {
              object obj;
              Guid iid = IID_IPersistFile;
              if (Native.CoCreateInstance(ref CLSID_ShellLink, IntPtr.Zero, 0x1, ref iid, out obj) != 0)
                  return null;
              try {
                  var pf = (IPersistFile)obj;
                  if (pf.Load(lnkPath, 0) != 0) return null;          // STGM_READ
                  var ps = (IPropertyStore)obj;
                  var pv = new PropVariant();
                  try {
                      if (ps.GetValue(ref PKEY_AppUserModel_ID, pv) != 0) return null;
                      return (pv.vt == 31 && pv.pwszVal != IntPtr.Zero)
                          ? Marshal.PtrToStringUni(pv.pwszVal) : null;
                  } finally { pv.Clear(); }
              } finally { Marshal.ReleaseComObject(obj); }
          }
      }
  }
  '@
  }

  try {
      if (-not (Test-Path -LiteralPath $LnkPath)) {
          Write-Warning "set_aumid: shortcut not found: $LnkPath"
      } else {
          [QMKonnect.ShortcutAumid]::Set($LnkPath, $Aumid) | Out-Null
          Write-Host "set_aumid: set System.AppUserModel.ID='$Aumid' on $LnkPath"
      }
  } catch {
      # Non-fatal: AUMID affects only notification branding. NEVER abort the install.
      Write-Warning "set_aumid: failed to set AUMID on $LnkPath : $_"
  }
  exit 0
  ```

  - NAMING: namespace `QMKonnect`, class `ShortcutAumid` (matches the `-as [type]` guard). Methods
    `Set`/`Get` (PascalCase, C# convention). File snake_case (`set_aumid.ps1`, repo style).
  - LINE-LENGTH / here-string: PowerShell `@'…'@` single-quoted here-string = no interpolation
    (the C# is literal). Keep the C# verbatim — every GUID, offset, and the Commit→Save order matter.
  - ERROR POSTURE: the `try/catch` + `exit 0` means the script NEVER fails the install. The Inno
    `[Code]` and install.ps1 callers can treat a non-zero exit as impossible; they still log defensively.
  - PLACEMENT: packaging/windows/inno/set_aumid.ps1 (same dir as QMKonnect.iss → the `[Files]`
    Source "set_aumid.ps1" resolves relative to the .iss).

Task 2: EDIT packaging/windows/inno/QMKonnect.iss — bundle + invoke the helper

  STEP 2a — [Files] add ONE line (bundle the helper to {tmp}, delete after install).
  Locate the existing [Files] block (the three Source lines):
      Source: "{#ReleaseDir}\qmkonnect.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
      Source: "{#AssetDir}\Icon.ico";          DestDir: "{app}"; Flags: ignoreversion
      Source: "{#AssetDir}\IconTray-dark.png"; DestDir: "{app}"; Flags: ignoreversion
  Append IMMEDIATELY AFTER the IconTray-dark.png line (still inside [Files], before [Icons]):
      ; Install-time only helper: sets System.AppUserModel.ID on the Start Menu shortcut so WinRT
      ; toasts render as "QMKonnect" (P1.M4.T2.S1). Extracted to {tmp} and deleted after install
      ; (deleteafterinstall) — never a runtime asset in {app}. Invoked by CurStepChanged below.
      Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall
  - DestDir: "{tmp}" is MANDATORY (compiler errors without DestDir). Source is RELATIVE TO THE .iss
    DIR → resolves to packaging/windows/inno/set_aumid.ps1 (created in Task 1).
  - PRESERVE: the three existing Source lines and all comments. Do NOT add set_aumid.ps1 to {app}.

  STEP 2b — [Code] add the CurStepChanged event procedure (set the AUMID post-[Icons]).
  Add this procedure INSIDE the existing [Code] section (e.g. right after KillRunningInstance's
  `end;`, before InitializeSetup). Inno recognizes event functions by NAME — no registration:

      // (P1.M4.T2.S1) Set System.AppUserModel.ID on the Start Menu shortcut so WinRT toasts render
      // as "QMKonnect" (must match src/platforms/mod.rs::APP_AUMID). ssPostInstall runs AFTER [Icons]
      // created the .lnk — the only ordering fact we need. (The [Run] app-launch may have already
      // fired; harmless — toasts trigger only on a post-startup window-focus rules.toml parse error.)
      // The AUMID literal below MUST equal APP_AUMID ("Mulletware.QMKonnect") byte-for-byte. Failure
      // is NON-FATAL (notification branding only): log and continue, never abort the install.
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

  - PATTERN: mirrors KillRunningInstance's `Exec(ExpandConstant(…), '…', '', SW_HIDE,
    ewWaitUntilTerminated, ResultCode)` exactly (lowest cognitive load).
  - AUMID LITERAL: "Mulletware.QMKonnect" — MUST equal src/platforms/mod.rs:134 APP_AUMID. (Inno
    Pascal cannot read the Rust const; the literal is intentional and cross-referenced in the comment.)
  - SW_HIDE hides the PowerShell console (matches KillRunningInstance). ewWaitUntilTerminated is
    synchronous within the step (no `nowait` race).
  - PRESERVE: [Icons] (line 93), [Run] (line 101-105, the app-launch entry), [Setup], [Registry].
    Do NOT add a [Run] entry for this (it would raise an error dialog on non-zero exit; [Code] is cleaner).

Task 3: EDIT packaging/windows/install.ps1 — dev-loop parity (call the shared helper after Save)

  install.ps1 creates the .lnk via WScript.Shell (lines ~60-66) with NO AUMID. The .iss header
  says it "Replicates ../install.ps1 exactly" — keep the invariant by calling the shared helper.
  Locate the block (after `$s.Description = '…'` / `$s.Save()`):
      $s.Description = 'QMKonnect - window-change notifier for QMK keyboards'
      $s.Save()
  Add IMMEDIATELY AFTER `$s.Save()` (and before the `# Default-on autostart …` comment):
      # (P1.M4.T2.S1) Set the AUMID on the Start Menu shortcut so WinRT toasts render as
      # "QMKonnect" — must equal src/platforms/mod.rs::APP_AUMID. Non-fatal: a failure only
      # degrades notification branding, never blocks install. Helper is shared with the Inno
      # installer (packaging/windows/inno/set_aumid.ps1) — keeps the two installers in sync.
      $Aumid = 'Mulletware.QMKonnect'
      $Helper = Join-Path $PSScriptRoot 'inno\set_aumid.ps1'
      try {
          & powershell.exe -NoProfile -ExecutionPolicy Bypass -File $Helper $StartMenu $Aumid
      } catch {
          Write-Warning "install.ps1: failed to set AUMID on shortcut (non-fatal): $_"
      }
  - $StartMenu is already defined (install.ps1 line ~60: `Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$App.lnk"`).
  - $PSScriptRoot = packaging/windows/, so `inno\set_aumid.ps1` resolves to the shared helper (Task 1).
  - ERROR POSTURE: `try/catch` + `Write-Warning` — install.ps1 has `$ErrorActionPreference='Stop'`;
    without the try/catch a helper failure would abort the install. AUMID is branding-only.
  - AUMID LITERAL: 'Mulletware.QMKonnect' (single source of truth; matches the .iss + APP_AUMID).
  - PRESERVE: everything else in install.ps1 (autostart Run key, ARP entry, launch, etc.).

Task 4: EDIT packaging/windows/inno/README.md — Mode A docs (AUMID requirement + verification)

  STEP 4a — add a bullet to "## What it does" (after the "Start Menu shortcut (manual launch)"
  bullet, line ~20). Insert AFTER that line:
      - sets the `System.AppUserModel.ID` (`Mulletware.QMKonnect`) on that Start Menu shortcut —
        required for Windows **toast notifications** to render (e.g. the "rules.toml invalid"
        toast); without it the toast is silently suppressed. Done by a post-install PowerShell
        helper (`set_aumid.ps1`), so it applies to both the installer and `install.ps1`.

  STEP 4b — add a verification command to "## Verifying the install" (after the autostart-value
  block, before "## Notes"). Append:
      ```bash
      # Start Menu shortcut advertises the AUMID (required for toasts). Expect: Mulletware.QMKonnect
      powershell -NoProfile -ExecutionPolicy Bypass -Command "& {
          . ([scriptblock]::Create((Get-Content -Raw '$LOCALAPPDATA/Programs/QMKonnect/../…' 2>\$null))) 2>\$null
      }" 2>/dev/null  # see note
      ```
      Simpler reliable form (PowerShell directly):
      ```powershell
      $lnk = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\QMKonnect.lnk"
      $h = Join-Path $env:LOCALAPPDATA 'Programs\QMKonnect\set_aumid.ps1'   # not shipped; for dev check
      # Production check: re-run the helper's Get() if present, or use the Shell extended property:
      (Get-Item $lnk).VersionInfo | Out-Null   # placeholder; the authoritative read is set_aumid.ps1 Get()
      ```
      > Authoritative verification: run `set_aumid.ps1`'s read-back, or (in a dev shell)
      > `[QMKonnect.ShortcutAumid]::Get($lnk)` → `Mulletware.QMKonnect`.

  NOTE: the bash verbatim above is intentionally a guide; the README is markdown and the
  authoritative read-back is `set_aumid.ps1`'s `Get()` helper (or the Shell COM property-store
  read). Keep the README note SHORT and user-facing: state the requirement (AUMID on the .lnk),
  that the installer sets it automatically, and give the simplest reliable verification. Do NOT
  inline the full C#. Anchor: insert after the `(Get-ItemProperty 'HKCU:\…\Run' …)` block (line ~114).

  - TONE: user-facing, factual, no PRP/task IDs in the README prose (the parenthetical "P1.M4.T2.S1"
    is for traceability — keep it minimal or drop it per repo style). Match surrounding markdown.

Task 5: VALIDATE (no edits — Linux regression + deferred Windows gates)
  - git diff --stat                # Expected: EXACTLY 4 files — set_aumid.ps1 (new), QMKonnect.iss,
                                    #   install.ps1, README.md. ZERO Rust files, ZERO Cargo.toml.
  - cargo build                    # Expected: unchanged-green (no Rust touched). Proves no accidental edits.
  - cargo test --bin qmkonnect -- --test-threads=1   # Expected: green, count UNCHANGED.
  - (DEFERRED to Windows, AGENTS.md loop — see Validation Level 3) build.ps1 → install → verify the
    .lnk AUMID = "Mulletware.QMKonnect" → break rules.toml → confirm a toast renders.

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT edit any Rust file (src/**, Cargo.toml, etc.). This is a packaging-only change. The
    APP_AUMID const (mod.rs:134) is READ-ONLY — the .iss/.ps1 hardcode the matching literal.
  - DO NOT use `tauri-winrt-notification`'s `PowerShell::create_shortcut` or create the .lnk at
    runtime (P1.M4.T1 chose Approach A; the installer owns the shortcut). Approach C does NOT apply.
  - DO NOT use the Inno `AppId` GUID ({FAAE1F7A-…}) as the AUMID — AUMID is "Mulletware.QMKonnect".
  - DO NOT change [Icons], [Run], [Setup], or [Registry] in QMKonnect.iss (the .lnk is correct;
    only annotate it post-hoc via [Code]). Do NOT add a [Run] entry for AUMID (use [Code] CurStepChanged).
  - DO NOT omit IPersistFile.Save (Commit alone does NOT persist the .lnk — #2 failure cause).
  - DO NOT use LayoutKind.Sequential for PROPVARIANT (wrong padding → silent fail / AV). Explicit + offsets only.
  - DO NOT call CoInitializeEx from the PowerShell helper (PS5.1 is STA, auto-inited; risks RPC_E_CHANGED_MODE).
  - DO NOT let set_aumid.ps1 exit non-zero, or let a helper failure abort install.ps1 (AUMID is branding-only).
  - DO NOT edit build.ps1 (it just runs iscc), the WiX installer.wxs (separate Session-0 service path),
    or docs/installation.md (Mode A = no change there).
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
```

### Implementation Patterns & Key Details

```powershell
# PATTERN (Inno [Code] Exec — mirrors QMKonnect.iss KillRunningInstance verbatim):
procedure CurStepChanged(CurStep: TSetupStep);
var ResultCode: Integer; …
begin
  if CurStep <> ssPostInstall then Exit;
  …
  if not Exec(ExpandConstant('{sys}\WindowsPowerShell\v1.0\powershell.exe'),
       PsArgs, '', SW_HIDE, ewWaitUntilTerminated, ResultCode) then
    Log('… non-fatal')
  else if ResultCode <> 0 then
    Log('… non-fatal');
end;

# PATTERN (C# PROPVARIANT — the #1 failure cause; copy the EXACT offsets):
[StructLayout(LayoutKind.Explicit)]
internal struct PropVariant {
    [FieldOffset(0)]  public ushort vt;        // VARTYPE
    [FieldOffset(8)]  public IntPtr pwszVal;   // VT_LPWSTR pointer (union first member)
    [FieldOffset(16)] public IntPtr pad;        // → 24 bytes on x64
}
// VT_LPWSTR = 31. Built via PropVariant.FromString(s) → vt=31, pwszVal=StringToCoTaskMemUni(s).

# PATTERN (the AUMID literal is pinned in 3 places — all must be "Mulletware.QMKonnect"):
#   1. src/platforms/mod.rs:134  pub const APP_AUMID: &str = "Mulletware.QMKonnect";   (read-only, S1)
#   2. QMKonnect.iss [Code]      PsArgs := … '" "Mulletware.QMKonnect"';                (Task 2b)
#   3. install.ps1               $Aumid = 'Mulletware.QMKonnect'                        (Task 3)
# A mismatch = toast silently dropped. Cross-reference in comments at sites 2 and 3.

# WHY [Code] CurStepChanged over a [Run] entry: matches the existing Exec idiom, non-fatal Log
#   handling (a [Run] entry raises an error dialog on non-zero exit), runs unconditionally across
#   /VERYSILENT (no skipifsilent analogue), and only needs "runs after [Icons]" (guaranteed).

# WHY always exit 0 + try/catch: AUMID affects only notification branding. A failure must NEVER
#   block an install. The Inno [Code] logs defensively; install.ps1 wraps in try/catch.

# WHY {sys}\WindowsPowerShell\v1.0\powershell.exe: canonical, always-present on Win10/11; avoids
#   PATH / app-execution-alias ambiguity of bare `powershell`. Matches best practice.

# WHY no CoInitializeEx: PowerShell 5.1 runs STA and auto-inits COM. Manual CoInitializeEx risks
#   RPC_E_CHANGED_MODE. The C# helper must NOT call it. (Contrast S2's Rust toast worker, which
#   DOES CoInitializeEx on a fresh thread because the event-loop thread may hold an MTA apartment.)
```

### Integration Points

```yaml
INNO [FILES]:
  - add to: packaging/windows/inno/QMKonnect.iss [Files]
  - pattern: 'Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall'
INNO [CODE]:
  - add to: QMKonnect.iss [Code]
  - pattern: procedure CurStepChanged(CurStep: TSetupStep); … Exec(powershell.exe … -File "{tmp}\set_aumid.ps1" "{userprograms}\QMKonnect.lnk" "Mulletware.QMKonnect") …
INSTALL.PS1:
  - add to: packaging/windows/install.ps1 (after $s.Save())
  - pattern: & powershell.exe -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot 'inno\set_aumid.ps1') $StartMenu 'Mulletware.QMKonnect'
README (MODE A):
  - add to: packaging/windows/inno/README.md ("What it does" + "Verifying the install")
AUMID SINGLE SOURCE OF TRUTH (3 pinned sites, all "Mulletware.QMKonnect"):
  - src/platforms/mod.rs:134 APP_AUMID (S1, READ-ONLY)
  - QMKonnect.iss [Code] (Task 2b)
  - install.ps1 (Task 3)
CONSUMES (from S1+S2, already committed):
  - the toast call in mod.rs:239 (CreateToastNotifierWithId(&APP_AUMID) → Show) renders ONLY once
    this task sets the .lnk AUMID. APP_AUMID const unchanged.
UNINSTALL:
  - automatic — Inno deletes [Icons] shortcuts (default); the AUMID lives on the .lnk, so it is
    removed with the .lnk. No [UninstallRun] / CurUninstallStepChanged needed.
SIBLING / PARALLEL (no conflicts):
  - P1.M4.T1.S1/S2 (preceding, committed): edited Cargo.toml + mod.rs + main.rs + troubleshooting.md.
    T2.S1 touches ONLY packaging/* + README. Zero Rust overlap. Merge clean.
  - P1.M3.T2.S1 (parallel): edits src/core/notifier.rs ONLY. Zero overlap.
PLATFORM VALIDATION:
  - Linux dev box: git diff --stat (no Rust) + cargo build/test unchanged-green. Cannot run
    ISCC/PowerShell/Windows COM.
  - Windows: DEFERRED — build.ps1 → install → verify .lnk AUMID → toast renders (AGENTS.md loop).
```

## Validation Loop

> Toolchain: this PRP is **packaging-only** (`.iss`, `.ps1`, `.md`). The implementing
> agent runs on **Linux** and CANNOT run ISCC, PowerShell, or Windows COM. The Linux
> gates prove *"no Rust was touched"* (regression baseline). The real gates run on
> Windows (AGENTS.md dev loop) — marked DEFERRED.

### Level 1: Scope / Build Hygiene (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat
# Expected: EXACTLY 4 files — packaging/windows/inno/set_aumid.ps1 (NEW),
#   packaging/windows/inno/QMKonnect.iss, packaging/windows/install.ps1,
#   packaging/windows/inno/README.md. ZERO Rust files. ZERO Cargo.toml.
# If ANY src/** or Cargo.toml appears → you overstepped scope; revert it.
git diff -- src/ Cargo.toml        # Expected: EMPTY
```

### Level 2: Rust regression baseline (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo build
# Expected: unchanged-green. Proves no accidental Rust edit. (No new deps/features involved.)
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL pass, test count UNCHANGED from baseline. (Single-threaded — AGENTS.md.)
# If a previously-passing test now fails → you accidentally touched a Rust file; revert.
```

### Level 3: Static review of the packaging files (Linux — runs, eyeball)
```bash
cd /home/dustin/projects/qmkonnect
# (a) The helper exists and pins the literal:
grep -n 'Mulletware.QMKonnect' packaging/windows/inno/set_aumid.ps1   # the param + doc reference (literal is passed in)
grep -n 'Add-Type\|ShortcutAumid\|IPropertyStore\|IPersistFile\|LayoutKind.Explicit\|FieldOffset(8)\|pf.Save' packaging/windows/inno/set_aumid.ps1
# (b) The .iss bundles + invokes it; AUMID literal matches:
grep -n 'set_aumid.ps1\|CurStepChanged\|Mulletware.QMKonnect\|deleteafterinstall' packaging/windows/inno/QMKonnect.iss
grep -n 'userprograms' packaging/windows/inno/QMKonnect.iss            # the [Icons] .lnk name (untouched)
# (c) install.ps1 calls the shared helper; literal matches:
grep -n "set_aumid.ps1\|Mulletware.QMKonnect\|Aumid" packaging/windows/install.ps1
# (d) README documents it:
grep -n 'AppUserModel\|AUMID\|toast' packaging/windows/inno/README.md
# (e) Three-way AUMID literal consistency (the single-source-of-truth check):
diff <(grep -o 'Mulletware.QMKonnect' src/platforms/mod.rs | sort -u) \
     <(grep -o 'Mulletware.QMKonnect' packaging/windows/inno/QMKonnect.iss | sort -u) \
     <(grep -o "Mulletware.QMKonnect" packaging/windows/install.ps1 | sort -u)
# Expected: identical "Mulletware.QMKonnect" in all three (diff exits 0 / no output).
```

### Level 4: Windows build + install + AUMID verification + toast render (DEFERRED — AGENTS.md loop)
```bash
# Run on a Windows host, from the CANONICAL path (Z:\projects\qmkonnect), NOT the C:\projects junction
# (AGENTS.md trap #2). Verify %CARGO_TARGET_DIR% is empty (trap #1) beforehand.
cd /z/projects/qmkonnect

# (a) Build the installer (needs Inno Setup 6: winget install JRSoftware.InnoSetup).
cargo build --release
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\inno\build.ps1
# Expected: packaging\windows\inno\Output\QMKonnect-Setup.exe. If ISCC errors on the new [Files]
#   line ("Required parameter DestinationDir missing" / "Source file not found") → check DestDir
#   is "{tmp}" and set_aumid.ps1 is at packaging/windows/inno/ (same dir as the .iss).

# (b) Install (interactive, in your own session).
taskkill /IM qmkonnect.exe /F   # release the single-instance mutex (AGENTS.md)
start "" "Z:\packaging\windows\inno\Output\QMKonnect-Setup.exe"
# Complete the wizard. The [Code] CurStepChanged runs set_aumid.ps1 (hidden) near the end.

# (c) Verify the AUMID is set on the Start Menu .lnk.
powershell -NoProfile -Command "dot-source the helper or inline its Get(); [QMKonnect.ShortcutAumid]::Get(\"$env:APPDATA\Microsoft\Windows\Start Menu\Programs\QMKonnect.lnk\")"
# Expected: Mulletware.QMKonnect. If it returns null/empty → set_aumid.ps1 failed silently;
#   check the installer log (View → Setup Log) for the CurStepChanged Log() line.

# (d) End-to-end toast render (needs S1+S2 landed — both are committed at HEAD).
#     Break rules.toml, then switch window focus to trigger host_context_for_window's re-parse.
$rules = "$env:APPDATA\QMKonnect\rules.toml"   # (confirm path via the app's -v log)
Add-Content $rules "`n= = garbage syntax = ="  # force a parse error
# Switch focus to another window and back. Expected: a toast "QMKonnect: rules.toml invalid"
# slides in, auto-dismisses after ~7s, and is reviewable in Action Center. No modal, no focus steal.
# If NO toast and the app's -v log shows show_toast succeeded → the .lnk AUMID is missing (re-check c).
# If NO toast and -v shows "show_toast: toast failed: …" → S2-side issue, not this task.

# (e) install.ps1 parity check (dev loop).
& "$env:LOCALAPPDATA\Programs\QMKonnect\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES   # uninstall first
powershell -NoProfile -ExecutionPolicy Bypass -File packaging\windows\install.ps1
# Re-run the (c) verification — the AUMID must be set identically. If not → install.ps1's helper
#   call (Task 3) is missing/wrong, or the Join-Path $PSScriptRoot 'inno\set_aumid.ps1' didn't resolve.

# (f) Uninstall cleanliness.
& "$env:LOCALAPPDATA\Programs\QMKonnect\unins000.exe" /VERYSILENT /SUPPRESSMSGBOXES
# Expected: the Start Menu QMKonnect.lnk is gone (Inno auto-removes [Icons] shortcuts). No residue.
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux: `git diff --stat` = exactly the 4 packaging/docs files; ZERO Rust / Cargo.toml.
- [ ] Linux: `cargo build` + `cargo test --bin qmkonnect -- --test-threads=1` unchanged-green (count identical).
- [ ] Windows (DEFERRED): `build.ps1` produces `QMKonnect-Setup.exe`; install runs `set_aumid.ps1` (checkable in the setup log); `.lnk` AUMID verifies as `Mulletware.QMKonnect`; a broken `rules.toml` renders a real toast.

### Feature Validation
- [ ] `set_aumid.ps1` exists; `param($LnkPath,$Aumid)`; Add-Type re-run guard; `::Set`; `::Get`; `exit 0`.
- [ ] C# PROPVARIANT is `LayoutKind.Explicit` (`vt`@0, `pwszVal`@8, `pad`@16 = 24 bytes; VT_LPWSTR=31); calls `Commit()` AND `IPersistFile.Save()`.
- [ ] `QMKonnect.iss` `[Files]` bundles `set_aumid.ps1` to `{tmp}` (`deleteafterinstall`); `[Code] CurStepChanged(ssPostInstall)` invokes it non-fatally.
- [ ] `install.ps1` calls the shared helper after `$s.Save()` (try/catch, non-fatal).
- [ ] AUMID literal `"Mulletware.QMKonnect"` is identical in `src/platforms/mod.rs:134` (read-only ref), `QMKonnect.iss`, and `install.ps1`.
- [ ] README documents the AUMID-shortcut requirement + verification command.

### Code Quality Validation
- [ ] `set_aumid.ps1` follows repo `.ps1` conventions (param block, `$ErrorActionPreference`, comment header like install.ps1/build.ps1).
- [ ] Inno `[Code]` mirrors the existing `KillRunningInstance` Exec idiom (same show/wait/resultcode shape).
- [ ] Failure is non-fatal everywhere (AUMID is branding-only): `exit 0` + `try/catch` + `Log`.
- [ ] Comments cross-reference the AUMID single source of truth and the P1.M4 chain (S1→S2→T2.S1).
- [ ] No new dependencies; no version bumps; no Rust changes; no docs/installation.md change (Mode A).

### Documentation & Deployment
- [ ] README is user-facing (the requirement is auto-handled by the installer); verification command included.
- [ ] The AUMID-vs-Inno-AppId-GUID distinction is documented (comments) so a future editor doesn't "fix" it wrongly.

---

## Anti-Patterns to Avoid

- ❌ Don't create the `.lnk` at runtime / use `tauri-winrt-notification` (Approach C) — P1.M4.T1 chose Approach A (raw `windows` crate); the installer owns the shortcut.
- ❌ Don't use the Inno `AppId` GUID as the AUMID — it's `"Mulletware.QMKonnect"` (matches `APP_AUMID`).
- ❌ Don't touch ANY Rust file, `Cargo.toml`, `[Icons]`, `[Run]`, `[Setup]`, `[Registry]`, `build.ps1`, the WiX `installer.wxs`, or `docs/installation.md`.
- ❌ Don't use `LayoutKind.Sequential` for PROPVARIANT — `Explicit` with `vt`@0 / `pwszVal`@8 / `pad`@16 is mandatory.
- ❌ Don't skip `IPersistFile.Save` — `IPropertyStore.Commit` alone does NOT persist the `.lnk`.
- ❌ Don't call `CoInitializeEx` from the PowerShell helper (PS5.1 is STA; risks `RPC_E_CHANGED_MODE`).
- ❌ Don't let `set_aumid.ps1` exit non-zero, or let a helper failure abort `install.ps1` — AUMID is branding-only.
- ❌ Don't add a `[Run]` entry for the AUMID step — use `[Code] CurStepChanged(ssPostInstall)` (non-fatal Log handling; a `[Run]` entry raises an error dialog on non-zero exit).
- ❌ Don't trust the subagent's `inno_postinstall_patterns.md` ordering claim (`ssPostInstall` before `[Run]`) — it's backwards; `research/inno_ordering_verified.md` corrects it. (Doesn't affect correctness — we only need "after [Icons]".)
- ❌ Don't claim the helper runs before the app launches — it doesn't (it's harmless; toasts need a post-startup window-focus event).
- ❌ Don't omit the `Add-Type` re-run guard (`-as [type]`) — the 2nd invocation throws without it.

---

## Confidence Score: 8/10

The task is well-bounded (4 packaging/docs files, zero Rust, no new deps), the
technical recipe (C# `IPropertyStore` on an existing `.lnk`) is verified against
the Microsoft desktop-toast quickstart and the canonical GUIDs/offsets, and the
two non-obvious failure causes (PROPVARIANT `LayoutKind.Explicit`; `Commit` ≠
persist → need `IPersistFile.Save`) are spelled out. The Inno integration mirrors
the codebase's existing `Exec` idiom and depends only on the unambiguous
"`ssPostInstall` runs after `[Icons]`" guarantee (I independently corrected the
subagent's ordering error). The `install.ps1` parity keeps the documented
"replicates exactly" invariant intact.

The 2-point reservation is the **validation asymmetry**: the implementing agent
runs on Linux and cannot run ISCC, PowerShell, or Windows COM, so the actual
installer compile, the `.lnk` AUMID verification, and the end-to-end toast render
are DEFERRED to the Windows dev loop (AGENTS.md). The Linux gates prove "no Rust
touched" and three-way AUMID-literal consistency, which catch the most likely
authoring errors (scope creep, literal drift). The remaining residual risk is a
subtle C# COM-marshalling compile error on Windows (e.g. `[Out] PropVariant`
handling) — if it surfaces, `research/aumid_recipe.md` documents the exact
struct layout and the verified call order, and the `emoacht`/Microsoft-API-Code-Pack
references give a second source. If the Inno `[Files]` compile fails, the fix is
the mandatory `DestDir: "{tmp}"` (documented). The approach is sound; the
execution risk is purely the deferred Windows verification.