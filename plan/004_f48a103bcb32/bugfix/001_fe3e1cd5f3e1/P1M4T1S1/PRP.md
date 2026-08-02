# PRP — P1.M4.T1.S1: Register process AUMID + add toast API dependencies/features (Approach A)

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files edited (3):** `Cargo.toml`, `src/platforms/mod.rs`, `src/main.rs`.
> **Approach chosen: A** (extend the existing `windows = "0.52.0"` dep with 3 features).
> This is **infrastructure only**: it does NOT yet render a toast. It sets the
> process AUMID at startup and exposes the WinRT toast API surface for
> **P1.M4.T1.S2** (which replaces the modal `MessageBoxW` in `platforms::notify`
> with a real toast). The Start-Menu `.lnk` that advertises the AUMID is
> **P1.M4.T2.S1**'s scope (Inno installer).
>
> **The defect this closes (infra half):** bug-hunt **Finding #3** (PRD `h2.1`
> #3) — `platforms::notify` on Windows is a focus-stealing modal `MessageBoxW`
> instead of a spec-mandated toast. The research
> (`architecture/windows_notify_research.md` §5) established the two
> prerequisites a real toast needs: (1) a registered process **AUMID**, and (2) a
> Start Menu shortcut advertising that AUMID. This task delivers (1) + the API
> surface; (2) + the actual toast call are downstream siblings.

---

## Goal

**Feature Goal**: Make a WinRT toast *possible* on Windows by (a) setting the
process-wide **AppUserModelID** (`"Mulletware.QMKonnect"`) at startup via
`SetCurrentProcessExplicitAppUserModelID`, and (b) enabling the three `windows`
crate features that expose the toast/AUMID API surface
(`Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom`) on the already-present
`windows = "0.52.0"` dependency. Expose the AUMID as a `pub const` so the
downstream Start-Menu-shortcut task (P1.M4.T2.S1) can reference the single
source of truth.

**Deliverable** (exactly three files):
1. **`Cargo.toml`** — add `Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom`
   to the `windows` dep's `features` list inside
   `[target.'cfg(target_os = "windows")'.dependencies]` (with an explanatory
   comment block).
2. **`src/platforms/mod.rs`** — add `pub const APP_AUMID: &str`, a compile-time
   const assertion that it is non-empty, and a `#[cfg(target_os = "windows")] pub
   fn set_aumid()` that calls `SetCurrentProcessExplicitAppUserModelID`.
3. **`src/main.rs`** — call `platforms::set_aumid()` (Windows-gated) in `main()`
   immediately after `init_logging()`, before `run()`.

**Success Definition**:
- On the **Linux dev box**: `cargo build` succeeds (the resolver validates the 3
  feature names are real for `windows 0.52.0`); `cargo test --bin qmkonnect --
  --test-threads=1` is green with **no change to the test count** (this task adds
  no runtime tests — only a compile-time const check that compiles everywhere).
  `git diff --stat` shows exactly the three files above.
- On **Windows** (deferred to AGENTS.md dev loop, the implementing agent runs on
  Linux so cannot compile the gated code): `cargo build --release` resolves
  `windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID` and
  builds; the running process has its AUMID set (verified via Process Explorer's
  "AppUserModel ID" column or the `GetProcessExplicitAppUserModelID` cross-check
  — see Validation Level 5).

## User Persona (if applicable)

**Target User**: the Windows end user (and the downstream implementers of
P1.M4.T1.S2 / P1.M4.T2.S1). For the end user this task is invisible — it adds no
visible behavior — but it is the prerequisite that lets a future toast (vs. the
current modal popup) render in Action Center when `rules.toml` is broken.

**Use Case**: A user's `rules.toml` fails to parse. Today they get a focus-
stealing modal box. After S1→S2 land, they get a toast. S1 is the foundation:
the process must identify itself as `"Mulletware.QMKonnect"` before the toast
subsystem will accept/originate a notification for it.

**Pain Points Addressed**: removes the infra blocker (missing AUMID + missing
API features) that has forced the `MessageBoxW` stop-gap. See the author's own
admission in `src/platforms/mod.rs:150` ("a true toast needs an
AppUserModelID + Start Menu shortcut to render").

## Why

- **A WinRT toast will silently not render without an AUMID.** Windows' toast
  subsystem (`ToastNotificationManager`) keys notifications to the originating
  app's AppUserModelID. A process without an explicit AUMID inherits a generated
  one tied to the *executable path*, which has no Start Menu shortcut and no
  display identity → the toast is dropped (no error). Setting
  `SetCurrentProcessExplicitAppUserModelID("Mulletware.QMKonnect")` at startup
  gives the process a stable identity the (future) `.lnk` will advertise.
- **The API surface is not currently enabled.** The `windows` dep is pulled in
  with 12 Win32 features, but none expose `windows::UI::Notifications::*` (the
  toast manager) or `windows::Data::Xml::Dom::*` (the toast-XML builder), nor
  `windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID` (the AUMID
  setter). S2 cannot compile without these features; S1 adds them.
- **Single source of truth for the AUMID.** Defining `pub const APP_AUMID` here
  means S2 (toast call) and T2.S1 (Inno `.lnk`) reference one value, eliminating
  the "AUMID drift" failure mode where the process sets one string and the
  shortcut advertises another (→ toast silently dropped).

## What

### Approach selection: A (windows-crate features) — not B (tauri-winrt-notification)

The contract offered both. **Choose A.** Documented rationale (see
`research/findings.md`):
1. **Existing pattern.** The codebase already depends on `windows = "0.52.0"`
   (12 features) and uses it in `windows.rs`, `mod.rs`, `autostart.rs`. Extending
   an existing dep is lower-risk than a new crate.
2. **The plan's task split implies the Inno installer owns the shortcut.** T1.S1
   (AUMID+deps) + T2.S1 ("Add AUMID property to Start Menu shortcut in
   QMKonnect.iss"). The Inno installer (`QMKonnect.iss:93`) already creates the
   `.lnk`; T2.S1 adds `System.AppUserModel.ID` there. Approach B's runtime
   `PowerShell::create_shortcut` would conflict with / duplicate T2.S1.
3. **No PowerShell runtime dependency.** Approach B shells out to PowerShell on
   first toast — a footgun for a background tray daemon.
4. **Scope.** One deduped toast per broken `rules.toml` (research §2/§3) doesn't
   justify a new crate.

### DOCS (Mode A): NONE
Per the contract, Approach A requires **no** `docs/installation.md` change. (Mode
B would have added a one-line note about the toast crate; we are not in Mode B.)

### Success Criteria
- [ ] `Cargo.toml`: the `windows` dep's `features` array (Windows target) contains
      `Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom` (each with a
      `#`-comment explaining its purpose), and no feature is removed.
- [ ] `src/platforms/mod.rs`: `pub const APP_AUMID: &str = "Mulletware.QMKonnect";`
      exists, with a compile-time `const _: () = assert!(!APP_AUMID.is_empty());`
      assertion adjacent to it.
- [ ] `src/platforms/mod.rs`: `#[cfg(target_os = "windows")] pub fn set_aumid()`
      calls `windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID`
      with the AUMID encoded as a UTF-16 wide string.
- [ ] `src/main.rs::main()`: `platforms::set_aumid()` is invoked
      (`#[cfg(target_os = "windows")]`-gated) after `init_logging()` and before
      `run()`.
- [ ] The `MessageBoxW` block in `notify()` (mod.rs:150-166) is **untouched**
      (that's P1.M4.T1.S2's job).
- [ ] Linux: `cargo build` succeeds; `cargo test --bin qmkonnect -- --test-threads=1`
      green with unchanged test count; `git diff --stat` = the 3 files.

## All Needed Context

### Context Completeness Check
_Pass._ An agent with no prior knowledge can implement all three edits from the
exact anchors below (the `windows` features array in Cargo.toml lines 66-79; the
`#[cfg(not(any(...)))]` no-op arm that closes `notify()` in mod.rs; `main()` in
main.rs), the verified API path + signature, and the AGENTS.md gates. The only
non-trivial judgment call (approach selection) is already made and justified
above. The validation asymmetry (Linux agent can't compile the Windows-gated
code) is explicitly documented as a deferred gate.

### Documentation & References

```yaml
# MUST READ — the bug-hunt research that established the AUMID + shortcut prerequisites
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/windows_notify_research.md
  why: "§5 enumerates the EXACT missing windows features and the two prerequisites a real toast
        needs (AUMID + Start Menu shortcut). §1 pins the single trigger (rules.toml parse error
        in host_context_for_window) and the dedup (at most once per broken state) — do not change
        either. §6 shows Linux/macOS already do the right thing; only Windows deviates."
  section: "§5 What toast/notification APIs are available in the dependency tree"
  critical: "the prerequisite list (AUMID + .lnk) is the whole reason this task exists. The
        process-level AUMID (SetCurrentProcessExplicitAppUserModelID, THIS task) and the
        .lnk AUMID (System.AppUserModel.ID, T2.S1) are BOTH required for a toast to render —
        this task delivers only the first."

# MUST READ — the implementing-agent's own research notes (decision + verified API paths)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S1/research/findings.md
  why: "documents the Approach A vs B decision (with rationale), the exact windows-rs feature
        names + API paths (Win32_UI_Shell / UI_Notifications / Data_Xml_Dom), the
        SetCurrentProcessExplicitAppUserModelID signature, the AUMID-vs-Inno-AppId distinction,
        the validation asymmetry (Linux agent can't compile the gated code), and the cross-task
        coordination (no overlap with P1.M3.T2.S1; hand-off to S2/T2.S1)."
  critical: "AUMID = 'Mulletware.QMKonnect' (NOT the Inno AppId GUID). The two are different
        identities; do not use the GUID as the AUMID."

# MUST READ — the file with the AUMID const + set_aumid() + the (untouched) MessageBoxW arm
- file: src/platforms/mod.rs
  why: "add APP_AUMID + the compile-time const check + set_aumid() near the top of the
        platform-dispatch module (after the `mod` declarations / before `pub trait WindowMonitor`
        is a clean spot, OR right above `pub fn notify`). The MessageBoxW Windows arm of notify()
        (lines 150-166) MUST be left untouched — that is P1.M4.T1.S2's territory."
  pattern: "Windows-specific helpers in this file are `#[cfg(target_os = \"windows\")] pub fn …`;
        PCWSTR is already imported inside the notify() arm (windows::core::PCWSTR) — set_aumid()
        can import it locally the same way. The osa_string helper (#[cfg(target_os=\"macos\")])
        is the precedent for a platform-gated private fn living in mod.rs."
  gotcha: "do NOT move notify()'s Windows arm or 'fix' it — S2 replaces it. Do NOT add the AUMID
        as a field on some struct; it is a process-global const."

# MUST READ — the Inno installer (defines app identity; T2.S1 will add the .lnk AUMID here)
- file: packaging/windows/inno/QMKonnect.iss
  why: "lines 25-26 define MyAppName='QMKonnect', MyAppPublisher='Mulletware' — the source of the
        'Mulletware.QMKonnect' AUMID (Publisher.App convention). Line 51 is the AppId GUID
        ({FAAE1F7A-…}) — that is the INSTALLER upgrade identity, NOT the AUMID. Line 93 is the
        Start Menu .lnk that T2.S1 will annotate with System.AppUserModel.ID."
  pattern: "do NOT edit this file in S1 (T2.S1 owns it). Read it only to confirm the AUMID spelling
        and to understand why the AUMID ≠ AppId."

# REFERENCE — the windows-crate usage idiom already in the repo (set_aumid() follows it)
- file: src/platforms/windows.rs
  why: "shows the established pattern for calling unsafe windows-rs functions inside `unsafe {}`
        with locally-imported `use windows::…` paths (e.g. the SetWinEventHook block at :62, the
        EnumChildWindows usage). set_aumid() mirrors this: local `use` inside the unsafe block."
  pattern: "`unsafe { use windows::core::PCWSTR; use windows::Win32::UI::Shell::…; let _ = Fn(args); }`"

# REFERENCE — where set_aumid() is called (main.rs), gated like the autostart module
- file: src/main.rs
  why: "main.rs:16 declares `#[cfg(target_os = \"windows\")] mod autostart;` — the exact gating
        idiom to mirror for the set_aumid() call. main() at :50 calls init_logging() first (keep
        that ordering), then run(). Insert the set_aumid() call between them."

# EXTERNAL — windows-rs feature hierarchy (confirms feature-name correctness)
- url: https://zread.ai/microsoft/windows-rs/11-the-windows-crate-safe-bindings
  why: "confirms each Cargo feature = one namespace module; enabling a child auto-enables parents;
        Win32_Foundation cascades (already present, do NOT re-add). Validates that Win32_UI_Shell,
        UI_Notifications, Data_Xml_Dom are the correct, minimal feature strings for windows 0.52."
  critical: "feature names are case- and underscore-sensitive; a typo (e.g. 'UI_Notification'
        singular, or 'Win32_UI_Shell_Properties') fails the cargo resolver. The 3 strings above are
        verified-correct namespace-module names."

# EXTERNAL — canonical 'send a Windows toast from Rust' recipe (consumed by S2, referenced here)
- url: https://microsoft.github.io/windows-docs-rs/doc/windows/UI/Notifications/struct.ToastNotificationManager.html
  why: "confirms ToastNotificationManager lives at windows::UI::Notifications (feature
        UI_Notifications). S2 will use CreateToastNotifier(&aumid_hstring); this task only enables
        the feature + sets the AUMID S2 will pass."
```

### Current Codebase tree (relevant slice)

```bash
# run from /home/dustin/projects/qmkonnect
Cargo.toml
  - :65-79  [target.'cfg(target_os="windows")'.dependencies] windows features list  ← EDIT (add 3 features)
src/platforms/mod.rs
  - :1-12   mod declarations (hyprland/linux/macos/windows/x11)
  - :14-30  pub trait WindowMonitor { … }                        ← add APP_AUMID + const-check + set_aumid() above or below notify()
  - :109-…  pub fn get_config_paths() …
  - :126-176 pub fn notify(title, body) { #[cfg(target_os="windows")] { … MessageBoxW … } }   ← DO NOT TOUCH the MessageBoxW arm
  - :255-…  #[cfg(test)] mod tests { … }
src/main.rs
  - :16     #[cfg(target_os="windows")] mod autostart;            ← gating idiom to mirror
  - :49-60  fn main() { init_logging()?; … run()? }               ← EDIT: insert set_aumid() call after init_logging, before run()
packaging/windows/inno/QMKonnect.iss                              ← DO NOT EDIT (T2.S1 owns it); read only for AUMID spelling
```

### Desired Codebase tree with files added/changed

```bash
Cargo.toml                      # +3 features in windows dep (Win32_UI_Shell, UI_Notifications, Data_Xml_Dom)
src/platforms/mod.rs            # +pub const APP_AUMID, +compile-time const assert, +#[cfg(windows)] pub fn set_aumid()
src/main.rs                     # +#[cfg(windows)] platforms::set_aumid(); call in main()
# (no new files; no new deps; no new test files)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (validation asymmetry): the implementing agent runs on LINUX. `cargo build`/`cargo test`
//   there compile ONLY the non-Windows code. The windows-target features and set_aumid()/
//   SetCurrentProcessExplicitAppUserModelID code are #[cfg(target_os="windows")] and are NOT
//   compiled on Linux. Two gates DO still run on Linux: (1) the cargo resolver validates the 3
//   feature names are real windows-0.52 features (a typo fails the build); (2) the compile-time
//   `const _: () = assert!(!APP_AUMID.is_empty());` references a plain &str so it compiles on
//   Linux too. The actual SetCurrentProcess* call + that the toast API paths resolve are validated
//   on WINDOWS (AGENTS.md Windows dev loop) — mark those DEFERRED.

// CRITICAL (AUMID ≠ Inno AppId): APP_AUMID = "Mulletware.QMKonnect" (Publisher.App). The Inno
//   `AppId` is the GUID {FAAE1F7A-…} — a DIFFERENT identity (installer upgrade tracking). Do NOT
//   use the GUID as the AUMID; toasts key off "Mulletware.QMKonnect" and the (future) .lnk must
//   advertise the SAME string.

// CRITICAL (toast needs BOTH an AUMID and a .lnk): this task sets the *process* AUMID. A WinRT
//   toast will still silently not render until a Start Menu .lnk advertises the AUMID — that is
//   P1.M4.T2.S1 (Inno installer). Do not expect a toast to appear after S1 alone.

// GOTCHA (SetCurrentProcessExplicitAppUserModelID is pure Win32, no COM): unlike the WinRT toast
//   APIs S2 will use, this shell32 function needs NO RoInitialize/CoInitializeEx. It is safe to
//   call on the main thread at startup. It is idempotent; calling it for subcommand paths
//   (--reload, --config) is harmless.

// GOTCHA (UTF-16 wide string + NUL terminator): windows-rs PCWSTR expects a NUL-terminated UTF-16
//   buffer. Build it as APP_AUMID.encode_utf16().chain(std::iter::once(0)).collect::<Vec<u16>>(),
//   then PCWSTR(buf.as_ptr()). The buffer must outlive the call — keep it in a local `let`.

// GOTCHA (do not touch notify()'s MessageBoxW arm, mod.rs:150-166): that is P1.M4.T1.S2's scope.
//   S1 only sets up infra; the modal stays until S2 replaces it.

// GOTCHA (feature hierarchy): Win32_Foundation is ALREADY in the features list and auto-cascades;
//   do NOT re-add it. Just append the 3 new namespace features. Feature strings are
//   case/underscore-sensitive (UI_Notifications, not UI_Notification).

// GOTCHA (windows-rs 0.52 is a Windows-TARGET-ONLY dep): the whole `windows` dep lives under
//   [target.'cfg(target_os = "windows")'.dependencies]. Adding features there does not affect the
//   Linux build at all. Adding APP_AUMID as an UNCONDITIONAL pub const is fine (plain &str, no
//   windows import) and lets the compile-time const check run on all platforms.

// GOTCHA (cross-task file ownership): P1.M3.T2.S1 (parallel) edits ONLY src/core/notifier.rs.
//   This task edits Cargo.toml + mod.rs + main.rs — ZERO overlap. S2 edits mod.rs notify() arm;
//   T2.S1 edits QMKonnect.iss. Merge in any order.
```

## Implementation Blueprint

### Data models and structure
None new. A single `pub const APP_AUMID: &str` (a process-identity string, not a
type). No structs, no enums, no config keys, no env vars.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT Cargo.toml — add 3 toast/AUMID features to the windows dep
  In [target.'cfg(target_os = "windows")'.dependencies], the windows dep's features array
  (starts at line 66 with `"Win32_Foundation",`) currently ends with:
      "    # Per-user \"Open at Login\" autostart via the HKCU `Run` key\n    # (src/autostart.rs; HANDOFF_WINDOWS_OPEN_AT_LOGIN.md).\n    \"Win32_System_Registry\",\n] }\n"
  Replace the closing with (appends the 3 features + a comment block, preserving Registry):
      "    # Per-user \"Open at Login\" autostart via the HKCU `Run` key\n    # (src/autostart.rs; HANDOFF_WINDOWS_OPEN_AT_LOGIN.md).\n    \"Win32_System_Registry\",\n    # WinRT toast infrastructure (P1.M4.T1 — spec/UI.md §2.3 mandates a toast, not\n    # the current modal MessageBoxW, for the \"rules.toml invalid\" notification):\n    #   - Win32_UI_Shell: SetCurrentProcessExplicitAppUserModelID (sets the process AUMID\n    #     at startup so the toast subsystem accepts notifications from this app).\n    #   - UI_Notifications: windows::UI::Notifications (ToastNotificationManager /\n    #     ToastNotification / ToastNotifier) — consumed by P1.M4.T1.S2.\n    #   - Data_Xml_Dom: windows::Data::Xml::Dom (XmlDocument to build the toast XML\n    #     payload) — consumed by P1.M4.T1.S2.\n    # Win32_Foundation is already present and auto-cascades; do not re-add it.\n    \"Win32_UI_Shell\",\n    \"UI_Notifications\",\n    \"Data_Xml_Dom\",\n] }\n"
  - ANCHOR NOTE: the oldText includes the `"Win32_System_Registry",\n] }\n` tail so it is unique
    (that exact Registry-comment block appears once). Keep the version pin `= ...`? NO — the dep
    line is `windows = { version = "0.52.0", features = [ … ] }`; we only edit inside `features`.
  - PRESERVE: every existing feature line (Win32_Foundation through Win32_System_Registry) and
    the existing autostart comment. Do NOT change the version or add a new dependency entry.

Task 2: EDIT src/platforms/mod.rs — add APP_AUMID const + compile-time check + set_aumid()
  Insert a new block. Best placement: immediately BEFORE the `pub fn notify(...)` doc comment
  (the function that currently owns the Windows MessageBoxW arm), so the AUMID + setter sit next
  to the code that will (via S2) consume them. Locate the anchor:
      "/// Best-effort, non-blocking desktop notification (fire-and-forget). Surfaces a\n/// malformed `rules.toml` to the user (HOST_RULES.md §7). The caller\n"
  and insert ABOVE it:
      "// ── Windows toast identity (P1.M4.T1.S1) ─────────────────────────────────────\n// Stable AppUserModelID for this app. A WinRT toast will not render unless the\n// process sets this AUMID (via set_aumid() below) AND a Start Menu shortcut\n// advertises it (P1.M4.T2.S1, Inno installer). Convention is Publisher.App; the\n// publisher/name mirror packaging/windows/inno/QMKonnect.iss (MyAppName=\n// \"QMKonnect\", MyAppPublisher=\"Mulletware\"). NOTE: this is NOT the Inno `AppId`\n// GUID ({FAAE1F7A-…}) — that is the installer upgrade identity; the AUMID is the\n// toast identity and the two are distinct.\npub const APP_AUMID: &str = \"Mulletware.QMKonnect\";\n\n// Compile-time guard: a blanked AUMID would silently break toasts at runtime;\n// fail the build instead. (Plain &str ⇒ compiles on every platform, so this also\n// gates the Linux dev build.)\n#[allow(dead_code)]\nconst _APP_AUMID_NONEMPTY: () = {\n    assert!(!APP_AUMID.is_empty());\n};\n\n/// Set this process's AppUserModelID so WinRT toasts originate from \"Mulletware.\n/// QMKonnect\" (must match the Start Menu shortcut's System.AppUserModel.ID).\n/// Call once at startup on Windows, before any toast. Pure Win32 shell32 — no\n/// COM init needed. Idempotent; failure is non-fatal (toasts just won't render).\n#[cfg(target_os = \"windows\")]\npub fn set_aumid() {\n    use windows::core::PCWSTR;\n    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;\n    // PCWSTR wants a NUL-terminated UTF-16 buffer; keep the Vec alive across the call.\n    let wide: Vec<u16> = APP_AUMID\n        .encode_utf16()\n        .chain(std::iter::once(0))\n        .collect();\n    let hr = unsafe {\n        SetCurrentProcessExplicitAppUserModelID(PCWSTR(wide.as_ptr()))\n    };\n    if let Err(e) = hr {\n        log::warn!(\"set_aumid: SetCurrentProcessExplicitAppUserModelID failed: {e}\");\n    }\n}\n\n"
  - NAMING: `APP_AUMID` (SCREAMING_SNAKE const), `set_aumid()` (snake_case fn) — matches repo style.
  - PRESERVE: the entire `notify()` body including its MessageBoxW Windows arm (S2's job).
  - GOTCHA: `log::warn!` resolves on Windows because `log` is a Windows-target dep (Cargo.toml).
    Do NOT use `eprintln!`/`info!` inconsistently; `warn!` on failure is the right level. `log` is
    imported at main.rs level on Windows but `log::warn!` (fully-qualified) needs no `use` here.

Task 3: EDIT src/main.rs — call set_aumid() early in main()
  In fn main() (around line 50), the current body is:
      "    // Initialize logging first\n    if let Err(e) = init_logging() {\n        eprintln!(\"Failed to initialize logging: {}\", e);\n        process::exit(1);\n    }\n\n    if let Err(e) = run() {\n"
  Replace with (insert the Windows-gated set_aumid() call between init_logging and run):
      "    // Initialize logging first\n    if let Err(e) = init_logging() {\n        eprintln!(\"Failed to initialize logging: {}\", e);\n        process::exit(1);\n    }\n\n    // Set the Windows process AppUserModelID BEFORE any toast could fire (P1.M4.T1.S1).\n    // A WinRT toast won't render unless the process identity is set; run() has many\n    // early-return subcommand branches, so set it here once for every path. Pure Win32,\n    // idempotent, non-fatal.\n    #[cfg(target_os = \"windows\")]\n    crate::platforms::set_aumid();\n\n    if let Err(e) = run() {\n"
  - GATING: `#[cfg(target_os = \"windows\")]` mirrors the `#[cfg(target_os = \"windows\")] mod autostart;`
    at main.rs:16. No non-Windows stub needed (the call site itself is gated out on other platforms).
  - PLACEMENT RATIONALE: after init_logging (so log::warn! inside set_aumid is wired), before run()
    (so every run() subcommand path has the AUMID set). Do NOT move it into run() or into
    WindowsMonitor::start() — process-global identity belongs at process start.

Task 4: VALIDATE (no edits)
  - cargo build                                  # Linux: resolver validates the 3 feature names;
                                                  #   the windows-target code is cfg-gated out.
  - cargo test --bin qmkonnect -- --test-threads=1   # Linux: green, test count UNCHANGED.
  - git diff --stat                              # exactly 3 files: Cargo.toml, src/platforms/mod.rs, src/main.rs.
  - (DEFERRED to Windows, AGENTS.md loop — see Validation Level 5) cargo build --release on the
    canonical path confirms SetCurrentProcessExplicitAppUserModelID + the toast features resolve.

Task 5: NEVER do these (out of scope / forbidden)
  - DO NOT touch notify()'s MessageBoxW Windows arm (mod.rs:150-166) — that is P1.M4.T1.S2.
  - DO NOT edit packaging/windows/inno/QMKonnect.iss — the Start Menu .lnk AUMID is P1.M4.T2.S1.
  - DO NOT use the tauri-winrt-notification crate (Approach B rejected; see Why).
  - DO NOT use the Inno AppId GUID as the AUMID — AUMID is "Mulletware.QMKonnect".
  - DO NOT add runtime tests for set_aumid (it calls a global OS setter; not unit-testable without a
    Windows process harness). The compile-time const check is the required test.
  - DO NOT add a non-Windows stub for set_aumid (the call site is cfg-gated; mirroring autostart).
  - DO NOT change docs/installation.md (Approach A → Mode A → no docs change, per contract).
  - DO NOT touch src/core/notifier.rs (P1.M3.T2.S1, parallel, owns it).
  - DO NOT bump the windows version, add new crates, or remove existing features.
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
```

### Implementation Patterns & Key Details
```rust
// PATTERN (windows-rs unsafe call, repo idiom from windows.rs:62/SetWinEventHook):
//   local `use` inside the unsafe scope, NUL-terminated wide buffer kept alive in a local `let`.
#[cfg(target_os = "windows")]
pub fn set_aumid() {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID;
    let wide: Vec<u16> = APP_AUMID.encode_utf16().chain(std::iter::once(0)).collect();
    let hr = unsafe { SetCurrentProcessExplicitAppUserModelID(PCWSTR(wide.as_ptr())) };
    if let Err(e) = hr { log::warn!("set_aumid: SetCurrentProcessExplicitAppUserModelID failed: {e}"); }
}

// COMPILE-TIME TEST (the only test this task adds; runs on ALL platforms since it touches a &str):
const _APP_AUMID_NONEMPTY: () = { assert!(!APP_AUMID.is_empty()); };
//   If APP_AUMID is ever blanked, the build fails at this const-eval. `assert!` in const is stable
//   since Rust 1.57; MSRV here is 1.88. `str::is_empty`/`len` are const-evaluable.

// WHY APP_AUMID is UNCONDITIONAL (not #[cfg(windows)]): it's a plain &str with no windows import,
//   so defining it on all platforms is free; making it unconditional lets the compile-time const
//   check run on the Linux dev box (a real gate the agent can exercise), and lets T2.S1/QMKonnect.iss
//   reference one value. Only set_aumid() (which imports windows) is cfg-gated.

// WHY set_aumid() is called in main(), not run()/WindowsMonitor::start(): run() has ~10 early-return
//   subcommand branches (--reload, --config, --list, --list-callbacks, …). Process-global identity
//   must be set once for ALL paths; main() (after init_logging) is the single chokepoint. Mirrors
//   how `mod autostart` is gated at main.rs:16.
```

### Integration Points
```yaml
CARGO:
  - add to: Cargo.toml → [target.'cfg(target_os = "windows")'.dependencies] → windows.features
  - pattern: append "Win32_UI_Shell", "UI_Notifications", "Data_Xml_Dom" (no version bump, no new dep)
IMPORTS:
  - mod.rs set_aumid(): local `use windows::core::PCWSTR; use windows::Win32::UI::Shell::…;`
  - main.rs: `crate::platforms::set_aumid()` (fully-qualified, no new `use`)
LOGGING:
  - set_aumid() uses log::warn! on failure (log is a Windows-target dep; init runs before the call)
CONFIG/ENV/CLI: NONE (no new env vars, config keys, or CLI flags)
PUBLIC API:
  - new pub const APP_AUMID (src/platforms/mod.rs) — consumed by P1.M4.T1.S2 (toast) + P1.M4.T2.S1 (.lnk)
  - new #[cfg(windows)] pub fn set_aumid() — consumed by main()
PARALLEL / SIBLING (no conflicts):
  - P1.M3.T2.S1 (parallel, in-flight): edits src/core/notifier.rs ONLY → zero file overlap.
  - P1.M4.T1.S2 (downstream): consumes the 3 features + APP_AUMID; replaces MessageBoxW in notify().
  - P1.M4.T2.S1 (downstream): references APP_AUMID in QMKonnect.iss Start Menu shortcut.
PLATFORM VALIDATION:
  - Linux dev box: cargo build (resolver checks feature names) + cargo test (const-check compiles,
    suite green). Cannot compile the windows-gated set_aumid/toast code.
  - Windows: deferred to AGENTS.md Windows dev loop (cargo build --release on canonical path).
```

## Validation Loop

> Toolchain: Rust (`cargo`). Tests MUST run single-threaded (AGENTS.md — shared
> global debouncer/mock state). **The implementing agent runs on Linux; the
> Windows-gated code (set_aumid + toast features) is NOT compiled there** — see
> the per-level notes. The Windows build is a DEFERRED gate (AGENTS.md loop).

### Level 1: Syntax & Style (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo build
# Expected: compiles. The cargo RESOLVER validates that Win32_UI_Shell, UI_Notifications,
#   Data_Xml_Dom are real windows-0.52 features (a typo/case error fails here even on Linux).
#   The windows-target code itself (set_aumid, the toast imports) is cfg-gated out and not compiled.
# If "unknown feature `…` for windows" → fix the feature name (case/underscore-sensitive).
# If "cannot find function `set_aumid`" on Linux → you forgot the #[cfg(target_os="windows")] gate
#   on the call site in main.rs (it must be cfg-gated exactly like `mod autostart`).
```

### Level 2: Unit Tests / The compile-time gate (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL pass, test count UNCHANGED from baseline (this task adds NO runtime tests). The
#   compile-time `const _APP_AUMID_NONEMPTY: () = { assert!(…) };` is evaluated during the build,
#   not as a #[test]. If APP_AUMID were blanked, the build would fail at const-eval (not reach here).
# Confirm the const gate actually fires (negative check, do NOT commit): temporarily set
#   APP_AUMID = "" → `cargo build` must error "assertion failed" at the const. Revert before done.
```

### Level 3: Full Suite Regression (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: same green count as before this change. No notify()-behavior change (MessageBoxW arm
#   untouched), no main()-control-flow change (set_aumid is a no-op on Linux), no Cargo.lock churn
#   beyond windows feature selection. If a previously-passing test now fails → you accidentally
#   touched notify() or reordered main(); revert and re-apply narrowly.
```

### Level 4: Feature-resolution spot check (Linux — runs, quick)
```bash
cd /home/dustin/projects/qmkonnect
cargo tree -p windows --target x86_64-pc-windows-msvc --no-default-features -e features 2>/dev/null \
  | grep -iE 'UI_Notifications|Data_Xml_Dom|Win32_UI_Shell' \
  || cargo metadata --format-version 1 2>/dev/null | grep -oE '"(Win32_UI_Shell|UI_Notifications|Data_Xml_Dom)"' | sort -u
# Expected: all three feature strings appear. (cargo tree with the windows target may not resolve
#   on a Linux host without the target installed; `cargo metadata` always reflects the resolved
#   features and is the reliable cross-host check.) If any is missing → Cargo.toml edit didn't land.
```

### Level 5: Windows build + AUMID-set verification (DEFERRED — AGENTS.md Windows dev loop)
```bash
# Run on a Windows host, from the CANONICAL path (Z:\projects\qmkonnect), NOT the C:\projects junction
# (AGENTS.md trap #2). Verify %CARGO_TARGET_DIR% is empty (trap #1) beforehand.
cd /z/projects/qmkonnect
cargo build --release
# Expected: resolves windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID and the
#   UI_Notifications / Data_Xml_Dom namespaces. If "cannot find function … in Win32::UI::Shell" →
#   feature name typo or wrong namespace (verify against research/findings.md table). If "unresolved
#   import windows::UI::Notifications" → UI_Notifications feature not enabled.
taskkill /IM qmkonnect.exe /F   # mandatory — single-instance mutex (AGENTS.md)
.\target\release\qmkonnect.exe -v   # run in your OWN session, NOT as a service
# AUMID-set verification (pick one):
#   (a) Sysinternals Process Explorer → View → Select Columns → add "AppUserModel ID" → find
#       qmkonnect.exe → its column reads "Mulletware.QMKonnect".
#   (b) From another shell:  PowerShell -NoProfile -c "Get-Process qmkonnect | ForEach-Object {
#         [QMKonnect.Native]::GetProcessExplicitAppUserModelId($_.Id) }" — or use the
#       GetProcessExplicitAppUserModelId P/Invoke pattern. The AUMID is "Mulletware.QMKonnect".
# NOTE: a toast will NOT render yet — this task only sets the AUMID + enables the API. The toast
#   itself is P1.M4.T1.S2; the required Start Menu .lnk is P1.M4.T2.S1. Expect: process AUMID set,
#   no visible toast. That is correct for S1.
```

### Level 6: Scope / Build Hygiene (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
git diff --stat                                # Expected: EXACTLY 3 files — Cargo.toml, src/platforms/mod.rs, src/main.rs
git diff src/core/notifier.rs                  # Expected: EMPTY (P1.M3.T2.S1 owns it)
git diff packaging/windows/inno/QMKonnect.iss  # Expected: EMPTY (P1.M4.T2.S1 owns it)
git diff docs/installation.md                  # Expected: EMPTY (Approach A → Mode A → no docs change)
grep -n 'APP_AUMID' src/platforms/mod.rs       # Expected: const def + 1 ref in set_aumid + 1 in const-check (>=3)
grep -n 'set_aumid' src/main.rs                # Expected: >=1 (the #[cfg(windows)] call)
grep -n 'Win32_UI_Shell\|UI_Notifications\|Data_Xml_Dom' Cargo.toml   # Expected: 3 matches in features
grep -n 'MessageBoxW' src/platforms/mod.rs     # Expected: still present (UNTOUCHED — S2 removes it later)
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux: `cargo build` succeeds (resolver validates the 3 feature names).
- [ ] Linux: `cargo test --bin qmkonnect -- --test-threads=1` green, test count unchanged.
- [ ] `git diff --stat` = exactly `Cargo.toml`, `src/platforms/mod.rs`, `src/main.rs`.
- [ ] Windows (DEFERRED, AGENTS.md loop): `cargo build --release` resolves the AUMID fn + toast namespaces; running process shows AUMID `Mulletware.QMKonnect`.

### Feature Validation
- [ ] `Cargo.toml`: `Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom` present in `windows` features (Windows target); no feature removed.
- [ ] `src/platforms/mod.rs`: `pub const APP_AUMID: &str = "Mulletware.QMKonnect";` + compile-time `assert!(!APP_AUMID.is_empty())` const check.
- [ ] `src/platforms/mod.rs`: `#[cfg(target_os = "windows")] pub fn set_aumid()` calls `SetCurrentProcessExplicitAppUserModelID` with a NUL-terminated UTF-16 AUMID, logging `warn!` on `Err`.
- [ ] `src/main.rs::main()`: `#[cfg(target_os = "windows")] crate::platforms::set_aumid();` runs after `init_logging()`, before `run()`.
- [ ] `notify()`'s MessageBoxW arm (mod.rs:150-166) is byte-for-byte unchanged (S2's job).
- [ ] Negative check: temporarily blanking `APP_AUMID` fails the build at the const-eval (verified, then reverted).

### Code Quality Validation
- [ ] Follows repo conventions: SCREAMING_SNAKE const, snake_case fn, local `use` inside unsafe, cfg-gated call site mirroring `mod autostart`.
- [ ] `set_aumid()` keeps the wide-string `Vec` alive across the `PCWSTR` call (no dangling pointer).
- [ ] Comments explain WHY (AUMID≠AppId GUID; toast needs both AUMID + .lnk; idempotent/non-fatal).
- [ ] No new dependencies; no version bumps; no docs/installation.md change (Mode A).

### Documentation & Deployment
- [ ] No user-visible behavior change (infra only) → no README/troubleshooting change.
- [ ] No new env vars / config keys / CLI flags.
- [ ] `APP_AUMID` documented as the single source of truth for S2 + T2.S1.

---

## Anti-Patterns to Avoid
- ❌ Don't pick Approach B (tauri-winrt-notification) — rejected for rationale in Why/findings.md (conflicts with T2.S1, adds a PowerShell runtime dep, over-scoped).
- ❌ Don't use the Inno `AppId` GUID as the AUMID — AUMID is `"Mulletware.QMKonnect"`.
- ❌ Don't touch `notify()`'s MessageBoxW arm, `QMKonnect.iss`, `src/core/notifier.rs`, or `docs/installation.md`.
- ❌ Don't add a non-Windows stub for `set_aumid` — the call site is cfg-gated (mirror `mod autostart`).
- ❌ Don't drop the wide-string `Vec` before the `PCWSTR` call (dangling pointer → UB).
- ❌ Don't call `set_aumid()` before `init_logging()` (the `warn!` on failure needs logging wired) or inside `run()`/`WindowsMonitor::start()` (misses subcommand paths).
- ❌ Don't add runtime tests for `set_aumid` (calls a global OS setter; the compile-time const check is the required test).
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared global state).
- ❌ Don't assume a toast will render after S1 alone — it needs S2 (the call) + T2.S1 (the .lnk). S1 is infra.
- ❌ Don't re-add `Win32_Foundation` (already present, auto-cascades) or change the `windows` version.

---

## Confidence Score: 8/10

The task is small, well-bounded (3 files, no new types, no runtime tests), and
the API paths/features are verified against the windows-rs feature hierarchy
docs (zread.ai/microsoft/windows-rs §2) + the canonical
`ToastNotificationManager` rustdoc (microsoft.github.io/windows-docs-rs). The
AUMID-vs-AppId distinction, the toast-needs-both-AUMID-and-.lnk requirement, and
the cross-task file ownership (no overlap with P1.M3.T2.S1; hand-off to
S2/T2.S1) are all documented. The 2-point reservation is the **validation
asymmetry**: the implementing agent runs on Linux and cannot compile the
Windows-gated `set_aumid()` / toast-feature code, so the actual
`SetCurrentProcessExplicitAppUserModelID` call site and the toast-namespace
resolution are validated only on Windows (AGENTS.md loop, deferred). The Linux
gates that DO run — the cargo resolver feature-name check and the compile-time
`APP_AUMID` const assertion — catch the two most likely authoring errors (feature
typos, blanked AUMID), which keeps the residual risk low. If the Windows build
surfaces a wrong-namespace error, the exact fix is in `research/findings.md`'s
API table.