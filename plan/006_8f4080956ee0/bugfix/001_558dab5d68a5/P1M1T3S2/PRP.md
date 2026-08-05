# PRP — P1.M1.T3.S2: Quote the Run-key path in `packaging/windows/inno/QMKonnect.iss` [Registry]

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`). ALL edits in ONE file:
> **`packaging/windows/inno/QMKonnect.iss`** (Bug 3B / PRD ID 4 — the
> **installer-side** Run-key writer). This is the installer half of the
> Windows-autostart quoting bug; the **app** half is P1.M1.T3.S1 (`src/autostart.rs`,
> in parallel). The two are independent (different files) but MUST produce the
> identical quoted value format.
> **Scope:** change ONE token on ONE line — the `[Registry]` Run-key `ValueData` —
> from unquoted to quoted using Inno's `""` escape. Nothing else.
> **⚠ iscc is Windows-only:** the Inno Setup compiler cannot run on the Linux dev
> box, so the .iss cannot be compile-checked here. Validate the TEXT on Linux; run
> the compile + registry check on a Windows host (mirrors S1's platform-gate split).

---

## Goal

**Feature Goal**: Make the Inno Setup installer (`QMKonnect-Setup.exe`) write the
HKCU `Run` autostart value as a **quoted** path so an install path containing spaces
(e.g. `C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe`) is stored as
the REG_SZ value `"C:\Users\John Doe\…\QMKonnect.exe"` — Windows then resolves it
correctly at login and it is no longer an unquoted-service-path vector. The format
MUST match what P1.M1.T3.S1's `current_exe_wide()` writes on the app side (a value
wrapped in literal `"`), so the installer and the in-app "Open at Login" toggle
produce byte-identical Run-key data.

**Deliverable**: `packaging/windows/inno/QMKonnect.iss` with the single `[Registry]`
Run-key line's `ValueData` changed from `"{app}\{#MyAppExeName}"` to
`"""{app}\{#MyAppExeName}"""` (Inno resolves `""` to one literal `"`). Everything
else on the line — `Root`, `Subkey`, `ValueType: string`, `ValueName: "QMKonnect"`,
`Flags: uninsdeletevalue` — is byte-identical. An optional one-line clarifying
comment note is recommended but not required by the contract.

**Success Definition**:
- On a **Windows** host: `build.ps1` (or `iscc QMKonnect.iss`) **compiles
  successfully** → `Output\QMKonnect-Setup.exe` is produced (iscc fails on a
  malformed string constant, so a clean compile is the primary proof the quote
  escaping is valid). Then a fresh install under a spaced path writes the Run key
  value `"…\QMKonnect.exe"` (literal quotes), verified by `reg query … /v QMKonnect`.
- On **any** host: the new line is textually correct (3 leading + 3 trailing `"`
  around `{app}\{#MyAppExeName}`; the rest of the line unchanged); the diff is
  limited to that one `ValueData` token (+ the optional comment).
- No Rust file, no other `.iss` line, no other packaging file is modified.

## User Persona (if applicable)

**Target User**: A Windows user whose profile path (username) or chosen install dir
contains a space.

**Use Case**: User runs `QMKonnect-Setup.exe`, which writes the default-on
autostart `Run` value. On reboot, QMKonnect actually launches. Before the fix, a
spaced path (`C:\Users\John Doe\…`) is written unquoted and Windows' login resolver
may mis-parse it (treating `C:\Users\John` as the exe) → autostart silently fails on
spaced usernames, AND the unquoted path is a privilege-escalation vector.

**User Journey**: (before) spaced install path → unquoted REG_SZ → login launch fails
silently OR is an exploit vector. (after) spaced install path → quoted REG_SZ →
launches reliably + no vector.

**Pain Points Addressed**: Silent autostart failure after install on spaced paths +
the unquoted-service-path security exposure (PRD Issue 3 / Major; bug_findings.md
§Bug 3B).

## Why

- **Login reliability + security on spaced Windows paths (installer half).** This is
  the INSTALLER-side writer of the same Run-key value that S1 fixes on the app side.
  The bug is only fully closed when BOTH write quoted values; if the installer writes
  unquoted and the app later re-writes quoted (or vice-versa), fresh installs still
  ship a vulnerable value until the user toggles autostart. Both must quote.
- **It's a one-token change.** The fix is confined to the `ValueData` of a single
  `[Registry]` entry. `ValueType`/`ValueName`/`Flags`/`Root`/`Subkey` are unchanged;
  `uninsdeletevalue` still deletes the value by name on uninstall (content-agnostic).
- **Cross-task consistency (contract requirement).** P1.M1.T3.S1 makes
  `current_exe_wide()` emit `"<path>"` (REG_SZ). This task makes the installer emit
  the identical `"…"` so the two writers never disagree. The shared contract is the
  value NAME `"QMKonnect"` (already identical) + the value FORMAT (quoted).

## What

A single edit to `packaging/windows/inno/QMKonnect.iss`. **Re-read the file before
editing** (line numbers can drift on later commits); match on the distinctive TEXT.

### Edit 1 — quote the `[Registry]` Run-key `ValueData`

The `[Registry]` section has exactly ONE entry (grep `insdeletevalue` → 1 hit). FIND
the exact current line (the distinctive fragment is `ValueData: "{app}\{#MyAppExeName}"`):

```ini
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "QMKonnect"; ValueData: "{app}\{#MyAppExeName}"; Flags: uninsdeletevalue
```

REPLACE WITH:

```ini
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "QMKonnect"; ValueData: """{app}\{#MyAppExeName}"""; Flags: uninsdeletevalue
```

> **The ONLY change** is `ValueData: "{app}\{#MyAppExeName}"` →
> `ValueData: """{app}\{#MyAppExeName}"""`. Everything before and after (the
> `Root`/`Subkey`/`ValueType`/`ValueName`/`Flags` tokens) is byte-identical.

### How Inno parses the new ValueData (the correctness argument)

Inno Setup string-constant rule: a string value is `"…"`; a literal `"` inside it is
written as two consecutive `"` (`""`). So `"""{app}\{#MyAppExeName}"""` is:

| chars | meaning |
|-------|---------|
| `"`   | opens the string literal |
| `""`  | ONE literal `"` (escaped) |
| `{app}\{#MyAppExeName}` | expanded path (`{app}` → install dir at install time; `{#MyAppExeName}` → `QMKonnect.exe` at compile time) |
| `""`  | ONE literal `"` (escaped) |
| `"`   | closes the string literal |

**Registry value written:** `"<expanded path>"` — e.g.
`"C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"`. That is exactly
the quoted REG_SZ format S1 produces app-side. ✓

> **Do NOT use a different escape.** Some Inno users try `'"'{app}…'"'` or
> `{encoded}` hacks — those are wrong. `""` is THE documented literal-quote escape;
> `"""…"""` (3 + 3) is the canonical pattern for "wrap this value in literal quotes".

### Edit 2 (OPTIONAL, recommended) — clarify the comment above the line

The existing comment (line 100-102) documents the Run-key contract (value name
"QMKonnect", shared with `src/autostart.rs` + `install.ps1`). It does NOT mention
quoting. A one-line note improves maintainability and ties the two writers together.
The contract says "DOCS: none" (no user-facing doc change), so this is a code-comment
tidy, NOT a doc deliverable — make it or skip it, both pass. FIND:

```ini
; Default-on autostart via the HKCU Run key. uninsdeletevalue removes it on
; uninstall. The value name "QMKonnect" is the CONTRACT shared with the tray
; toggle (src/autostart.rs) and ../install.ps1 - keep it identical everywhere.
```

REPLACE WITH (adds one sentence):

```ini
; Default-on autostart via the HKCU Run key. uninsdeletevalue removes it on
; uninstall. The value name "QMKonnect" is the CONTRACT shared with the tray
; toggle (src/autostart.rs) and ../install.ps1 - keep it identical everywhere.
; ValueData is QUOTED ("" = one literal ") so a spaced install path resolves at
; login and is not an unquoted-service-path vector - MUST match autostart.rs.
```

### Success Criteria

- [ ] The `[Registry]` Run-key line's `ValueData` is `"""{app}\{#MyAppExeName}"""`.
- [ ] `Root`, `Subkey`, `ValueType: string`, `ValueName: "QMKonnect"`,
      `Flags: uninsdeletevalue` are byte-identical to before.
- [ ] (Windows) `build.ps1` / `iscc QMKonnect.iss` compiles → `QMKonnect-Setup.exe`
      produced (clean compile = the escaping is valid).
- [ ] (Windows) fresh install on a spaced path → `reg query …Run /v QMKonnect` shows
      a quoted REG_SZ (`"…\QMKonnect.exe"` with literal quotes).
- [ ] (any host) the diff is limited to the one `ValueData` token (+ optional comment).
- [ ] No `.rs` file, no other `.iss` line, no other packaging file changed.

## All Needed Context

### Context Completeness Check

> _"If someone knew nothing about this codebase, would they have everything needed to
> implement this successfully?"_ — **Yes.** The exact buggy line (verbatim, line 103),
> the verbatim one-token replacement, the Inno quote-escaping derivation (the table
> proving `"""…"""` ⇒ `"<path>"`), the authoritative iscc doc URL, the cross-task
> consistency requirement with S1, the validation split (Linux grep vs Windows
> iscc+reg query), the out-of-scope `install.ps1` note, and the anti-patterns (don't
> quote `[Icons]`/`[Run]` Filename) are all below.

### Documentation & References

```yaml
# MUST READ — the file being edited (read current code before editing).
- file: /home/dustin/projects/qmkonnect/packaging/windows/inno/QMKonnect.iss
  why: "Contains the [Registry] section (line ~99-103) with the SINGLE Run-key entry
        to fix. Also confirms [Setup] DefaultDirName={localappdata}\\Programs\\… (the
        spaced-path source), [Icons] (line ~97, do NOT touch), and [Run] (line ~106,
        do NOT touch — it launches, doesn't write registry)."
  pattern: "[Registry] ValueData is written VERBATIM as REG_SZ — Inno does NOT
            auto-quote it (unlike [Icons]/[Run] Filename, which handle spaces
            internally). Manual \"\"\"…\"\"\" quoting is required HERE and ONLY here."
  gotcha: "Exactly ONE [Registry] entry (grep insdeletevalue → 1 hit). The change is
           the ValueData token only."

# MUST READ — the authoritative bug analysis + the exact fix prescribed.
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/architecture/bug_findings.md
  why: "§Bug 3 (B) prescribes the EXACT fix verbatim: change
        `ValueData: \"{app}\\{#MyAppExeName}\"` to `ValueData: \"\"\"{app}\\{#MyAppExeName}\"\"\"`.
        Confirms Inno uses \"\" for literal quotes. This PRP implements that
        recommendation as-is."
  section: "Bug 3 (Major, PRD ID 4) → Root Cause B + Fix B"
  critical: "The fix is the 3+3 quote pattern (\"\"\"…\"\"\"). Do not invent another
             escape; \"\" is THE documented literal-quote escape in Inno string constants."

# MUST READ — the sibling PRP (S1: the APP-side writer of the SAME Run-key value).
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T3S1/PRP.md
  why: "S1 (in parallel) makes src/autostart.rs::current_exe_wide() emit a QUOTED
        REG_SZ value: [0x0022, …path…, 0x0022, 0x0000] ⇒ registry value \"<path>\".
        This task MUST produce the identical quoted format so installer + app agree.
        DIFFERENT file (autostart.rs vs QMKonnect.iss) ⇒ no edit collision."
  section: "Goal, What-(a) (the [0x0022 …path… 0x0022 0x0000] layout)"
  critical: "Both writers must emit \"<path>\" (literal quotes). The value NAME
             (\"QMKonnect\") is already shared; the FORMAT is what this task aligns."

# REFERENCE — Inno Setup [Registry] section + ValueData semantics (the escape rule).
- url: https://jrsoftware.org/ishelp/index.php?topic=registrysection
  why: "Official [Registry] section docs. ValueData is 'the data to store'; for
        ValueType: string it is written verbatim as REG_SZ. Confirms Inno performs NO
        auto-quoting of the expanded path. The literal-quote escape (\"\") is stated
        in the help's string-constant/parameter-types overview."
  critical: "Use \"\"\"…\"\"\" (3+3 quotes) to wrap the value in literal quotes.
             A single pair of quotes around the path would just DELIMIT the string
             (and vanish) — you need the doubled quotes to EMIT literal quotes."

# REFERENCE — the bugfix PRD (severity + repro).
- file: /home/dustin/projects/qmkonnect/plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/prd_snapshot.md
  why: "Issue 3 (Major): repro = install on a spaced username, check HKCU\\…\\Run
        for the unquoted path. Confirms severity + the symptom (login failure)."
  section: "Major Issues → Issue 3"

# REFERENCE — how the installer is compiled (build.ps1 invokes iscc).
- file: /home/dustin/projects/qmkonnect/packaging/windows/inno/build.ps1
  why: "build.ps1 runs `iscc \"/DMyAppVersion=$V\" QMKonnect.iss` → Output\\QMKonnect-Setup.exe.
        iscc FAILS on a malformed string constant, so a clean compile is the primary
        validation that the quote escaping is valid (run on a Windows host). Needs
        `cargo build --release` first + Inno Setup 6 (`winget install JRSoftware.InnoSetup`)."

# REFERENCE — the Windows dev test loop (AGENTS.md).
- file: /home/dustin/projects/qmkonnect/AGENTS.md
  why: "Documents the Inno installer build: `powershell -NoProfile -ExecutionPolicy
        Bypass -File packaging\\windows\\inno\\build.ps1` → QMKonnect-Setup.exe. This
        is the command that compile-checks the .iss on Windows."

# REFERENCE — research notes for THIS subtask (escape derivation + iscc-unavailable + install.ps1 residual).
- docfile: plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M1T3S2/research/notes.md
  why: "The full quote-escape derivation table, the proof that Inno does NOT auto-quote
        [Registry] ValueData (the trap), the iscc-not-on-Linux constraint, and the
        out-of-scope install.ps1:102 residual (same bug, not in this contract)."
```

### Current Codebase tree (relevant slice)

```bash
qmkonnect/
├── src/
│   └── autostart.rs          # S1's scope (app-side Run-key writer) — DO NOT TOUCH
└── packaging/windows/
    ├── install.ps1           # PowerShell installer twin (line 102 SAME bug) — OUT OF SCOPE (see notes)
    └── inno/
        ├── QMKonnect.iss     # <-- EDIT (ONLY): [Registry] Run-key ValueData line ~103
        ├── build.ps1         # runs iscc → QMKonnect-Setup.exe (the compile gate, Windows)
        ├── README.md         # inno installer readme — DO NOT TOUCH
        ├── set_aumid.ps1     # install-time AUMID helper — DO NOT TOUCH
        └── Output/           # build output (gitignored) — QMKonnect-Setup.exe
```

### Desired Codebase tree with files to be modified

```bash
packaging/windows/inno/
└── QMKonnect.iss   # MODIFIED ONLY:
                    #   [Registry] Run-key ValueData: "{app}\{#MyAppExeName}"
                    #   → ValueData: """{app}\{#MyAppExeName}"""  (+ optional comment line)
# (no new files; autostart.rs is S1's; install.ps1 is out-of-scope; no .rs change)
```

### Known Gotchas of our codebase & Library Quirks

```ini
; CRITICAL: Inno Setup does NOT auto-quote [Registry] ValueData.
;   Unlike [Icons] Filename and [Run] Filename (which Inno launches/builds itself,
;   handling spaces internally), [Registry] ValueData is written VERBATIM as the
;   REG_SZ bytes. A spaced {app} therefore lands unquoted unless you manually wrap
;   it. The fix is the """…""" (3+3 quote) pattern — NOT a single pair of quotes
;   (a single pair just delimits the string and vanishes from the value).

; CRITICAL: use """{app}\{#MyAppExeName}""" — exactly 3 leading + 3 trailing quotes.
;   Parsing: " (open) + "" (one literal ") + {app}\{#MyAppExeName} + "" (one literal ")
;   + " (close) => registry value "<expanded path>". Any other quote count is wrong:
;     - 1+1 ("{app}…")  => value is the BARE path (the bug, unchanged).
;     - 2+2 (""{app}…"") => Inno sees "" (empty string) + {app}… junk => COMPILE ERROR.
;     - 4+4             => value is ""<path>"" (extra quotes) — wrong.
;   iscc catches 2+2 at compile time but NOT 4+4 (it compiles, ships a wrong value).
;   So COUNT the quotes: exactly 3 on each side.

; CRITICAL: iscc (the Inno compiler) is WINDOWS-ONLY and NOT on this Linux dev box.
;   You CANNOT compile-check the .iss here. Validate the TEXT (grep the new line,
;   count quotes) on Linux; run build.ps1/iscc on a Windows host for the real gate.

; CRITICAL: quote ONLY the [Registry] ValueData. Do NOT also quote:
;   - [Icons] Filename: "{app}\{#MyAppExeName}" (line ~97)  — Inno builds the .lnk;
;     quoting would break the shortcut target.
;   - [Run]   Filename: "{app}\{#MyAppExeName}" (line ~106) — Inno launches via
;     CreateProcess; quoting would break the launch.
;   These two parameter types handle spaces internally; only [Registry] ValueData
;   is verbatim and needs manual quoting.

; GOTCHA: uninsdeletevalue is unaffected by quoting. It deletes the Run value by NAME
;   ("QMKonnect") on uninstall, content-agnostic. A quoted data value is still the
;   value named "QMKonnect" and is still deleted. Do NOT change Flags.

; GOTCHA: the app-side is_enabled() (autostart.rs, S1) is presence-based (len > 0),
;   so a quoted value is still detected as "enabled". After BOTH S1 + S2, the
;   installer and the toggle write the SAME quoted format and never disagree.

; NOTE (OUT OF SCOPE): packaging/windows/install.ps1 line 102 has the SAME unquoted
;   Run-key bug (Set-ItemProperty -Path $RunKey -Name $App -Value $ExeDest). It is the
;   .iss's stated twin but is NOT in this task's contract (PRD §Bug 3 lists only
;   autostart.rs + QMKonnect.iss). Do NOT fix it here. Flagged as a known residual for
;   a human follow-up if that PowerShell path is still shipped.

; NOTE: this is a .iss (Inno Setup script), NOT Rust. cargo build/cargo test do NOT
;   compile or validate it. The default cargo build on Linux is a no-op for this edit
;   (regression-only: confirms nothing ELSE broke). Real validation = iscc (Windows).
```

## Implementation Blueprint

### Data models and structure

No data models. The only change is the text of one `ValueData` token in an Inno
Setup `[Registry]` directive. The registry value's logical shape changes from a bare
path to a quoted path (`"<expanded path>"`).

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: READ the current file + confirm the anchor
  - READ: packaging/windows/inno/QMKonnect.iss in full. Confirm the [Registry]
          section has exactly ONE entry (the Run-key line, ~line 103) and its
          ValueData is currently "{app}\{#MyAppExeName}" (unquoted).
  - CONFIRM (grep): `grep -c insdeletevalue QMKonnect.iss` => 1 (one [Registry] entry).
  - CONFIRM: [Icons] Filename (~line 97) and [Run] Filename (~line 106) also reference
          "{app}\{#MyAppExeName}" but are DIFFERENT parameter types — do NOT touch them.

Task 2: EDIT the [Registry] Run-key ValueData (Edit 1)
  - REPLACE the single line's ValueData token: "{app}\{#MyAppExeName}"
          => """{app}\{#MyAppExeName}""" (3 + 3 quotes). Keep Root/Subkey/ValueType/
          ValueName/Flags byte-identical. (Verbatim FIND/REPLACE in "What".)
  - COUNT the quotes after editing: exactly 3 immediately before {app} and 3
          immediately after the closing } of {#MyAppExeName}.

Task 3 (OPTIONAL): clarify the comment above the line (Edit 2)
  - IF you choose to: append one sentence to the existing comment (verbatim in "What")
          noting the path is quoted to handle spaces + match autostart.rs. Skip is
          also acceptable (contract: "DOCS: none").

Task 4: VALIDATE (do not skip)
  - ON LINUX (text sanity — iscc cannot run here):
      grep -n 'ValueData' packaging/windows/inno/QMKonnect.iss
        # EXPECT: ValueData: """{app}\{#MyAppExeName}"""  (3+3 quotes)
      grep -c 'ValueData: """{app}\\{#MyAppExeName}"""' packaging/windows/inno/QMKonnect.iss
        # EXPECT: 1
      git diff --stat   # EXPECT: only packaging/windows/inno/QMKonnect.iss
  - ON WINDOWS (the real gate — compile + registry):
      cargo build --release                          # build the exe the installer packages
      powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/inno/build.ps1
        # EXPECT: "Built: …\Output\QMKonnect-Setup.exe" (clean iscc compile = escaping valid)
      # Then install (ideally under a spaced username, or temporarily set a spaced
      # DefaultDirName) and verify the registry value is quoted:
      reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v QMKonnect
        # EXPECT: REG_SZ value shows "<path>\QMKonnect.exe" WITH literal quote chars
```

### Implementation Patterns & Key Details

```ini
; === WHY [Registry] ValueData needs manual quoting but [Icons]/[Run] do NOT ===
;   Inno Setup handles parameter values differently by type:
;     [Icons]  Filename  -> Inno creates the .lnk (resolves spaces itself)
;     [Run]    Filename  -> Inno launches via CreateProcess (resolves spaces itself)
;     [Registry] ValueData -> written VERBATIM as REG_SZ bytes (NO auto-quoting)
;   So a spaced {app} is fine for shortcuts/launches but breaks the Run-key value.
;   The """…""" pattern emits literal quotes into the REG_SZ so Windows' login
;   resolver parses the WHOLE quoted string as the exe path.

; === WHY 3+3 quotes (not 1+1 or 2+2) ===
;   Inno string: "…" delimits; "" inside = one literal ". To PRODUCE a value that
;   STARTS and ENDS with a literal ", you need: open-quote + ""(literal ") + content
;   + ""(literal ") + close-quote = 3 quotes on each side. 1+1 only delimits (bare
;   value); 2+2 is an empty-string-then-junk compile error; 4+4 over-quotes (compiles
;   but wrong). COUNT = 3 each side.

; === WHY match S1 exactly ===
;   S1 (autostart.rs) writes the Run value when the user toggles "Open at Login". This
;   task's installer writes the SAME value at install time (default-on autostart). If
;   they disagreed (one quoted, one bare), the value would flip format depending on
;   whether the user had ever toggled the setting. Both emit "<path>" so they agree.

; === WHY uninsdeletevalue still works ===
;   It deletes by VALUE NAME ("QMKonnect"), not by data content. Quoting the data
;   doesn't change the name, so uninstall still removes it. Flags unchanged.
```

### Integration Points

```yaml
SOURCE FILES:
  - modify: "packaging/windows/inno/QMKonnect.iss ONLY"
  - do NOT modify: "src/autostart.rs (S1's scope), packaging/windows/install.ps1
                    (OUT OF SCOPE — same bug at line 102, not in this contract),
                    any .rs file, any other .iss line, Cargo.toml, docs/* (separate sweep)"

REGISTRY:
  - HKCU\Software\Microsoft\Windows\CurrentVersion\Run
  - value name: "QMKonnect" (UNCHANGED)
  - value data: was {app}\{#MyAppExeName} (expanded bare path)
              => now "<expanded path>" (quoted)  e.g.
                 "C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"
  - effect: Windows login resolver parses the quoted string as one exe path; a spaced
            install dir no longer breaks autostart or exposes an unquoted-service-path
            vector.

BUILD / PACKAGING:
  - build.ps1 (unchanged) runs iscc on QMKonnect.iss; the new ValueData must compile
    cleanly under iscc (the compile is the gate that the """ escaping is valid).
  - Output\QMKonnect-Setup.exe is regenerated (gitignored); never commit it.

DEPENDENCIES:
  - none. No Cargo.toml change, no new tooling. (iscc/Inno Setup 6 is an existing
    build prerequisite, documented in AGENTS.md + build.ps1.)

VALIDATION CONSUMERS:
  - ON WINDOWS: build.ps1 (iscc compile) + reg query post-install = THE gates.
  - ON LINUX: grep/quote-count text check only (iscc unavailable). The default cargo
    build is a no-op for this .iss edit (regression-only).
```

## Validation Loop

> Commands run from the repo root `/home/dustin/projects/qmkonnect`. **The Inno
> compiler (iscc) is Windows-only and NOT installed on the Linux dev box**, so the
> .iss cannot be compile-checked here. Validate the TEXT on Linux; run the compile +
> registry check on a Windows host (mirrors S1's platform-gate split).

### Level 1: Text sanity (any host — the Linux-checkable gate)

```bash
cd /home/dustin/projects/qmkonnect

# Confirm the new ValueData has the 3+3 quote pattern.
grep -n 'ValueData' packaging/windows/inno/QMKonnect.iss
# Expected: exactly ONE line, showing: ValueData: """{app}\{#MyAppExeName}"""
#   Count the quotes: 3 immediately before {app}, 3 immediately after the final }.

# Exact-match the whole token (robust count check).
grep -c 'ValueData: """{app}\{#MyAppExeName}"""' packaging/windows/inno/QMKonnect.iss
# Expected: 1. (0 => you mis-counted the quotes or edited the wrong line.)

# Confirm the rest of the Run-key line is byte-identical (Root/Subkey/ValueType/
# ValueName/Flags unchanged).
grep -n 'insdeletevalue' packaging/windows/inno/QMKonnect.iss
# Expected: the line still reads ...ValueType: string; ValueName: "QMKonnect"; ...
#           Flags: uninsdeletevalue  (unchanged).

# Confirm ONLY the .iss changed.
git diff --stat
# Expected: only packaging/windows/inno/QMKonnect.iss listed.

# Confirm you did NOT accidentally quote the [Icons]/[Run] Filename lines.
grep -n 'Filename: "{app}' packaging/windows/inno/QMKonnect.iss
# Expected: the [Icons] and [Run] Filename lines STILL show "{app}\{#MyAppExeName}"
#   with a SINGLE pair of quotes (unquoted-path style) — do NOT change these.
```

### Level 2: Regression — default Rust build still links (any host)

```bash
cd /home/dustin/projects/qmkonnect
# The .iss edit does NOT touch Rust, so this is a pure "nothing else broke" check.
cargo build 2>&1 | tail -2
# Expected: "Finished `dev` profile …" (no warnings, no errors). A .iss edit cannot
#   break this; running it confirms you didn't accidentally edit a .rs file.
```

### Level 3: Compile the installer (Windows host — THE gate)

```bash
cd /home/dustin/projects/qmkonnect
# Prereqs: cargo build --release (produces the exe the installer packages) + Inno
# Setup 6 (winget install JRSoftware.InnoSetup).
cargo build --release
powershell -NoProfile -ExecutionPolicy Bypass -File packaging/windows/inno/build.ps1
# Expected: "Built: …\packaging\windows\inno\Output\QMKonnect-Setup.exe" (green).
#   iscc FAILS the compile on a malformed string constant, so a clean build is the
#   primary proof the """ escaping is valid. If iscc errors with a string-constant
#   complaint, re-check the quote count (must be 3+3).
```

### Level 4: Registry verification on a spaced path (Windows host)

```bash
cd /home/dustin/projects/qmkonnect
# Install QMKonnect-Setup.exe. To exercise the spaced-path case without a spaced
# username, you can TEMPORARILY edit DefaultDirName to a spaced folder, rebuild the
# installer, install, then revert. OR test on a profile whose name has a space.
# After install, query the Run key:
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v QMKonnect
# Expected: the REG_SZ value data is the QUOTED path, e.g.
#     "C:\Users\John Doe\AppData\Local\Programs\QMKonnect\QMKonnect.exe"
#   (literal double-quote chars at the start and end). If you see the BARE path
#   (no surrounding quotes), the ValueData edit did not land — re-check it.

# Optional: confirm a REBOOT (or run > explorer restart) actually launches QMKonnect
# from the spaced path (the user-visible symptom this fix resolves). A process named
# QMKonnect should be running after login.
tasklist /FI "IMAGENAME eq qmkonnect.exe"
```

## Final Validation Checklist

### Technical Validation

- [ ] Level 1 (any host): `grep 'ValueData' QMKonnect.iss` shows
      `"""{app}\{#MyAppExeName}"""` (3+3 quotes); exact-match count = 1.
- [ ] Level 1: the rest of the Run-key line (Root/Subkey/ValueType/ValueName/Flags)
      is byte-identical; `git diff --stat` shows only `QMKonnect.iss`.
- [ ] Level 2: `cargo build` → Finished (no-Rust-change regression).
- [ ] Level 3 (Windows): `build.ps1` → `QMKonnect-Setup.exe` builds cleanly (iscc
      compile = escaping valid).
- [ ] Level 4 (Windows, spaced path): `reg query …Run /v QMKonnect` shows a QUOTED
      REG_SZ value.

### Feature Validation

- [ ] The `[Registry]` Run-key `ValueData` is `"""{app}\{#MyAppExeName}"""`.
- [ ] Fresh install + upgrade via `QMKonnect-Setup.exe` write a quoted path to HKCU Run.
- [ ] The quoted format matches P1.M1.T3.S1's app-side value (`"<path>"`) — installer
      and toggle now agree.
- [ ] `uninsdeletevalue` still removes the value on uninstall (name-unchanged).
- [ ] `[Icons]` / `[Run]` Filename lines are UNCHANGED (single-quote style preserved).

### Code Quality Validation

- [ ] The change is confined to one `ValueData` token (+ optional comment).
- [ ] No other `.iss` line, no `.rs` file, no other packaging file modified.
- [ ] The `install.ps1:102` twin bug is left alone (out of scope) but noted.

### Documentation & Deployment

- [ ] DOCS = none per contract (no user-facing doc change). The existing Run-key
      comment still documents the value-name contract; the optional Edit 2 adds a
      quoting note (code comment, not a doc deliverable).
- [ ] No Cargo.toml / config / environment-variable change.

---

## Anti-Patterns to Avoid

- ❌ Don't use a single pair of quotes (`"{app}\{#MyAppExeName}"`) — that only
  DELIMITS the Inno string and the value lands BARE (the bug, unchanged). You need
  the DOUBLED quotes (`""`) to EMIT literal quotes: `"""…"""` (3 each side).
- ❌ Don't mis-count the quotes. Exactly 3 immediately before `{app}` and 3
  immediately after `{#MyAppExeName}`'s closing `}`. 2+2 ⇒ iscc compile error
  (empty-string-then-junk); 4+4 ⇒ compiles but ships an over-quoted (wrong) value.
  iscc catches 2+2 but NOT 4+4, so COUNT them.
- ❌ Don't quote the `[Icons]` Filename (`"{app}\{#MyAppExeName}"`, ~line 97) or the
  `[Run]` Filename (~line 106). Those parameter types handle spaces INTERNALLY (Inno
  builds the .lnk / launches via CreateProcess); quoting them breaks the shortcut
  target and the post-install launch. Manual quoting is required for `[Registry]`
  ValueData ONLY (it's written verbatim as REG_SZ).
- ❌ Don't change `ValueType`, `ValueName`, `Flags`, `Root`, or `Subkey`. The contract
  is explicit: keep `ValueType: string`, `ValueName: "QMKonnect"`,
  `Flags: uninsdeletevalue` unchanged. Only the `ValueData` token changes.
  `uninsdeletevalue` is name-based, so quoting the data can't break uninstall.
- ❌ Don't expect to compile-check the `.iss` on the Linux dev box — `iscc` (Inno
  Setup) is Windows-only and not installed here. Validate the TEXT (grep + quote
  count) on Linux; run `build.ps1`/`iscc` + `reg query` on a Windows host. This is
  the platform-gate split, the installer analog of S1's `#![cfg(target_os="windows")]`
  situation.
- ❌ Don't run `cargo test` expecting it to validate the `.iss` — this is an Inno
  Setup script, not Rust. `cargo build`/`cargo test` do not touch it; the default
  build is regression-only (confirms no `.rs` file was accidentally edited).
- ❌ Don't fix `packaging/windows/install.ps1:102` here. It has the SAME unquoted
  Run-key bug (`Set-ItemProperty -Value $ExeDest`), but it is OUT OF SCOPE for this
  task (the PRD §Bug 3 + this contract cover only `autostart.rs` + `QMKonnect.iss`).
  Flag it for a human follow-up; do not expand the task.
- ❌ Don't edit `src/autostart.rs` — that's P1.M1.T3.S1's scope (the app-side writer).
  Different file; both tasks land independently but must emit the same quoted format.
- ❌ Don't invent a different escape (`'"'`, `%22`, `{encoded}`, backslash-quote) —
  Inno's string constants use `""` for a literal `"` and nothing else. Stick to the
  documented `"""…"""` pattern.
- ❌ Don't commit `Output\QMKonnect-Setup.exe` or any build artifact — they are
  gitignored and regenerated by `build.ps1`.
- ❌ Don't treat line numbers (~103) as contracts — match on the TEXT (the
  `ValueData: "{app}\{#MyAppExeName}"` token) when editing; a later commit could
  shift the line.

---

**Confidence Score: 9/10** for one-pass implementation success. The deliverable is a
single-token change to one `[Registry]` line of an Inno Setup script, with a
verbatim FIND/REPLACE, a table proving the `"""…"""` ⇒ `"<path>"` parse, and an
exact-match grep that confirms the quote count. The fix is prescribed verbatim in
bug_findings.md §Bug 3B and produces a value byte-identical to P1.M1.T3.S1's app-side
output (installer + toggle agree). The one residual risk — `iscc` being Windows-only
so the .iss cannot be compile-checked on the Linux dev box — is pre-empted by the
text-level grep/quote-count gate (runnable here) plus explicit Windows-host
`build.ps1`/`reg query` instructions (the real gate), mirroring S1's proven
platform-gate handling. The `install.ps1:102` twin bug is correctly identified as
out-of-scope (not in this contract). (Score 9 not 10 only because definitive
compile + registry verification requires a Windows host the Linux box cannot
provide — the escaping logic itself is verified by the parse derivation.)