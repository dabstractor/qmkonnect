# Research Notes — P1.M1.T3.S2: Quote the Run-key path in `QMKonnect.iss` [Registry]

## 1. The buggy line (verified, current HEAD)

`packaging/windows/inno/QMKonnect.iss`, `[Registry]` section, **line 103** (line 100 is
the comment above it):

```ini
; Default-on autostart via the HKCU Run key. uninsdeletevalue removes it on
; uninstall. The value name "QMKonnect" is the CONTRACT shared with the tray
; toggle (src/autostart.rs) and ../install.ps1 - keep it identical everywhere.
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "QMKonnect"; ValueData: "{app}\{#MyAppExeName}"; Flags: uninsdeletevalue
```

The ONLY defect is `ValueData: "{app}\{#MyAppExeName}"` — the expanded path is written
to the HKCU `Run` key **unquoted**. The fix changes JUST the ValueData:
`ValueData: """{app}\{#MyAppExeName}"""`.

There is exactly ONE `[Registry]` entry (grep `insdeletevalue` → 1 hit, line 103). The
`[Run]` section (line 106) LAUNCHES the app post-install and does NOT write a registry
value — out of scope, must not be touched.

## 2. Why the path can contain spaces

`[Setup]` → `DefaultDirName={localappdata}\Programs\{#MyAppName}` (line ~55). Expanded:

```
C:\Users\<username>\AppData\Local\Programs\QMKonnect\QMKonnect.exe
```

`<username>` comes from the Windows profile. If it contains a space (`John Doe`,
`Jane Smith`), the unquoted `Run` value is the textbook "Unquoted Service Path"
defect: Windows' login resolver may parse `C:\Users\John` as the exe (and `Doe\…` as
args) → autostart silently fails AND a writable ancestor dir becomes a privilege-
escalation vector. (PRD Issue 3 / Major; bug_findings.md §Bug 3B.)

## 3. Inno Setup quote-escaping — THE correctness point

Inno Setup string-constant rules (official help, [Registry] topic + parameter-types
overview):

- A parameter's string value is enclosed in `"…"`.
- To embed a literal `"` inside that string, write TWO consecutive `"`: `""`.

So `ValueData: """{app}\{#MyAppExeName}"""` parses as:

| chars | meaning |
|-------|---------|
| `"`   | opens the string literal |
| `""`  | one literal `"` (escaped) |
| `{app}\{#MyAppExeName}` | expanded path text (`{app}` at install time → install dir; `{#MyAppExeName}` at compile time → `QMKonnect.exe`) |
| `""`  | one literal `"` (escaped) |
| `"`   | closes the string literal |

**Result written to the registry:** `"<expanded path>"` → e.g.
`"C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"`.

That is EXACTLY the format P1.M1.T3.S1 produces on the app side
(`current_exe_wide()` → `[0x0022, …path…, 0x0022, 0x0000]` ⇒ REG_SZ value
`"<path>"`). So after both tasks, the installer AND the in-app "Open at Login"
toggle write the identical quoted value. (Contract requirement: "This MUST match
P1.M1.T3.S1's quoting.")

Authoritative refs:
- Inno Setup Help, [Registry] section (ValueData semantics):
  https://jrsoftware.org/ishelp/index.php?topic=registrysection
- The quote-doubling rule is stated across the help ("use two consecutive double-
  quote characters to include a single double-quote"). This `"""…"""` pattern is the
  canonical way to quote an exe path in a Run-key ValueData (widely used; e.g. many
  Inno Setup GitHub installer scripts).

## 4. GOTCHA: Inno does NOT auto-quote [Registry] ValueData

This is the subtle trap. Inno Setup DOES internally handle quoting/spaces for some
parameter types:
- `[Icons]` `Filename:` — Inno builds the .lnk target itself; spaces are handled.
- `[Run]` `Filename:` — Inno launches the exe via CreateProcess; spaces are handled.

BUT `[Registry]` `ValueData:` is written **verbatim** as the REG_SZ data bytes. Inno
performs NO quoting of the expanded path. So for a spaced install dir the unquoted
ValueData lands in the registry unquoted. **Manual quoting (`"""…"""`) is required
here and ONLY here.** Do NOT also quote the `[Icons]`/`[Run]` Filename lines — that
would double-break them (they'd try to launch a literal `"<path>"` including the
quote chars).

## 5. What stays unchanged (the contract is explicit)

`ValueType: string`, `ValueName: "QMKonnect"`, `Flags: uninsdeletevalue`, `Root:
HKCU`, `Subkey:` — ALL unchanged. Only the `ValueData:` token changes.

- `uninsdeletevalue` deletes the value by NAME on uninstall; quoting the DATA does
  not affect deletion (the value is still named "QMKonnect").
- The app-side `is_enabled()` (autostart.rs, S1) is presence-based (`len > 0`); a
  quoted value is still present + non-empty ⇒ still detected. So the tray checkbox
  still reflects the value correctly after BOTH the installer and the app write the
  quoted form.

## 6. Validation — split across hosts (iscc is Windows-only)

`iscc` (the Inno Setup compiler) is **NOT on this Linux dev box** (verified:
`command -v iscc` → not found). Inno Setup 6 is Windows-only. So:

- **On Linux (this box):** only a TEXT sanity-check is possible — grep the new line,
  count the quotes (must be 3 leading + 3 trailing around `{app}…`), confirm
  ValueType/ValueName/Flags/Root/Subkey are byte-identical. A malformed quote
  sequence is NOT caught here (iscc would catch it, but iscc can't run).
- **On a Windows host (the real gate):** `build.ps1` runs `iscc "/DMyAppVersion=$V"
  QMKonnect.iss`. `iscc` FAILS the compile on a malformed string constant, so a
  successful `Output\QMKonnect-Setup.exe` build is the primary proof the escaping is
  valid. Then install (ideally under a spaced username, or temporarily set a spaced
  DefaultDirName) and `reg query "HKCU\…\Run" /v QMKonnect` → the REG_SZ must show
  `"<path>"` (literal quotes present).

This mirrors S1's situation: the artifact under test is platform-specific and the
definitive validation runs on Windows. On Linux we verify the TEXT and the
no-Rust-change regression (`cargo build` is a no-op for a .iss edit — it doesn't
touch the Rust build at all).

## 7. Related, OUT-OF-SCOPE: `install.ps1:102` has the SAME bug

`packaging/windows/install.ps1` (the `.iss`'s stated twin — the `.iss` comment line 16
says "Replicates ../install.ps1 exactly") writes the SAME Run key unquoted at line 102:

```powershell
Set-ItemProperty -Path $RunKey -Name $App -Value $ExeDest   # $ExeDest = unquoted path
```

The PRD (§Bug 3) and this task's contract scope ONLY `src/autostart.rs` (S1) +
`QMKonnect.iss` (S2). `install.ps1` is NOT in the contract. **Do NOT fix it here** —
it is out of scope and would expand the task beyond its 0.5-point contract. Flagged
as a known residual: if the PowerShell installer path is still shipped (AGENTS.md
lists `install.ps1` as a documented path under Windows packaging), the unquoted bug
persists there. The human should decide whether to open a follow-up. (The shipped/
primary installer per AGENTS.md + build.ps1 is the Inno `QMKonnect-Setup.exe`, which
THIS task fixes.)

## 8. No Rust change ⇒ no cargo test relevance

This task edits a `.iss` (Inno Setup script), not Rust. `cargo build` / `cargo test`
do NOT compile or validate the `.iss`. The default `cargo build` on this box is a
pure regression check (confirms nothing ELSE broke) — it is unaffected by the .iss
edit. The real validation is iscc (Windows) + reg query (Windows). State this
explicitly so the implementer doesn't waste time looking for a Rust test to run.

## 9. DOCS per contract: none

The contract says "DOCS: none — no user-facing surface change. The installer comment
already documents the Run key contract." So NO doc files change. The existing comment
on line 100-102 already documents the Run-key contract + the name "QMKonnect". A
brief inline note that the path is quoted (to handle spaces + match autostart.rs) is
RECOMMENDED for maintainability but not required by the contract; present it as an
optional tidy, not a deliverable.