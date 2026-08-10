; ====================================================================================
;  QMKonnect - tray-app installer (Inno Setup 6)
; ====================================================================================
;  Produces QMKonnect-Setup.exe: the per-user, NO-ADMIN installer for the
;  *interactive tray app* (menu-bar icon + "Open at Login" toggle). This is the
;  installer to ship to end users - they double-click it and get the standard
;  Next -> Next -> Finish wizard.
;
;  Deliberately separate from ../installer.wxs (WiX), which builds an MSI that
;  installs a Session-0 *service*. A service runs in Session 0, which CANNOT
;  render a tray icon in the user's interactive session (see ../../AGENTS.md),
;  so it is the wrong vehicle for the tray app.
;
;  Replicates ../install.ps1 exactly:
;    * copies qmkonnect.exe + icon assets to {localappdata}\Programs\QMKonnect
;    * Start Menu shortcut (manual launch)
;    * HKCU Run value "QMKonnect" - default-on autostart, the SAME single source
;      of truth the in-app "Open at Login" toggle manages (src/autostart.rs)
;    * launches the app after an interactive install
;    * registers an Add/Remove-Programs uninstall entry
;
;  Build: .\build.ps1  (needs Inno Setup 6; winget install JRSoftware.InnoSetup)
; ====================================================================================

#define MyAppName      "QMKonnect"
#define MyAppPublisher "Mulletware"
#define MyAppURL       "https://github.com/dabstractor/qmkonnect"
#define MyAppExeName   "QMKonnect.exe"

#ifndef MyAppVersion
  #define MyAppVersion "0.2.8"
#endif

; Built exe: CARGO_TARGET_DIR\release if set, else ../../../target/release
; relative to this .iss.
;
; IMPORTANT (untrusted-mount foot-gun, see build.ps1): this path is baked into
; the installer at COMPILE time and re-read at install time. If it lands on a
; VM-shared / network volume (e.g. the Z:\ host mount on the dev VM), Windows
; rejects the read with STATUS_UNTRUSTED_MOUNT_POINT ("The path cannot be
; traversed because it contains an untrusted mount point"). When the project
; lives on such a mount, set CARGO_TARGET_DIR to a system-drive path (e.g.
; C:\qmk-target) in the shell that runs BOTH `cargo build --release` and
; build.ps1. build.ps1 validates this and aborts before producing a broken
; installer.
#ifndef ReleaseDir
  #if Len(GetEnv("CARGO_TARGET_DIR")) > 0
    #define ReleaseDir GetEnv("CARGO_TARGET_DIR") + "\release"
  #else
    #define ReleaseDir "..\..\..\target\release"
  #endif
#endif

; Icon assets live two levels up in packaging/.
#define AssetDir "..\.."

[Setup]
; AppId is the STABLE upgrade identity - keep it constant across versions so
; reinstalls upgrade in place rather than installing side-by-side. The {{ is
; Inno's escape for a literal { , so {{GUID} is stored as {GUID}.
AppId={{FAAE1F7A-9DBD-4C2A-B122-A9A73F05D0B3}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
UninstallDisplayName={#MyAppName}
DefaultDirName={localappdata}\Programs\{#MyAppName}
; Fixed per-user location (matches install.ps1) - hide the folder picker.
DisableDirPage=yes
DisableProgramGroupPage=yes
; Per-user, NO UAC prompt (a tray app must run in the interactive session).
PrivilegesRequired=lowest
UsePreviousAppDir=yes
; Modern single-page look + small, self-contained output.
WizardStyle=modern
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
; Installer / ARP icon.
SetupIconFile={#AssetDir}\Icon.ico
UninstallDisplayIcon={app}\Icon.ico
; Only ship English; skip the language dialog.
ShowLanguageDialog=no
OutputDir=Output
OutputBaseFilename=QMKonnect-Setup

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
; KillRunningInstance() (see [Code]) runs in PrepareToInstall BEFORE this copy,
; otherwise Windows holds a lock on the running exe and the upgrade can't
; replace it (the app holds a single-instance named mutex).
Source: "{#ReleaseDir}\qmkonnect.exe"; DestDir: "{app}"; DestName: "{#MyAppExeName}"; Flags: ignoreversion
Source: "{#AssetDir}\Icon.ico";          DestDir: "{app}"; Flags: ignoreversion
Source: "{#AssetDir}\IconTray-dark.png"; DestDir: "{app}"; Flags: ignoreversion
; Install-time only helper: sets System.AppUserModel.ID on the Start Menu shortcut so WinRT
; toasts render as "QMKonnect" (P1.M4.T2.S1). Extracted to {tmp} and deleted after install
; (deleteafterinstall) - never a runtime asset in {app}. Invoked by CurStepChanged below.
Source: "set_aumid.ps1"; DestDir: "{tmp}"; Flags: deleteafterinstall

[Icons]
Name: "{userprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; WorkingDir: "{app}"; IconFilename: "{app}\Icon.ico"; Comment: "QMKonnect - window-change notifier for QMK keyboards"

[Registry]
; Default-on autostart via the HKCU Run key. uninsdeletevalue removes it on
; uninstall. The value name "QMKonnect" is the CONTRACT shared with the tray
; toggle (src/autostart.rs) and ../install.ps1 - keep it identical everywhere.
; ValueData is QUOTED ("" = one literal ") so a spaced install path resolves at
; login and is not an unquoted-service-path vector - MUST match autostart.rs.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "QMKonnect"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue

[Run]
; Launch after a successful *interactive* install only. skipifsilent keeps
; headless /VERYSILENT verification runs from spawning a tray-less background
; process (the app is a windows-subsystem tray app with no console).
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#MyAppName}}"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
// Force-close any running QMKonnect so its single-instance named mutex releases
// and the exe file can be overwritten. Mirrors install.ps1's
// `Get-Process QMKonnect | Stop-Process -Force`. taskkill returns non-zero when
// the process isn't running, which we simply ignore.
procedure KillRunningInstance();
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{cmd}'), '/C taskkill /IM qmkonnect.exe /F /T', '',
    SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

// (P1.M4.T2.S1) Set System.AppUserModel.ID on the Start Menu shortcut so WinRT toasts render
// as "QMKonnect" (must match src/platforms/mod.rs::APP_AUMID). ssPostInstall runs AFTER [Icons]
// created the .lnk - the only ordering fact we need. (The [Run] app-launch may have already
// fired; harmless - toasts trigger only on a post-startup window-focus rules.toml parse error.)
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
        IntToStr(ResultCode) + ') - non-fatal')
  else if ResultCode <> 0 then
    Log('CurStepChanged: set_aumid.ps1 exited ' + IntToStr(ResultCode) + ' - non-fatal');
end;

function InitializeSetup(): Boolean;
begin
  KillRunningInstance();
  Result := True;
end;

function InitializeUninstall(): Boolean;
begin
  KillRunningInstance();
  Result := True;
end;
