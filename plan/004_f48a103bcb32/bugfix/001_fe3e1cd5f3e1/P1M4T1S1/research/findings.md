# Research Notes — P1.M4.T1.S1: Register process AUMID + add toast API deps/features

## Item (contract summary)
Set the process AUMID (`"Mulletware.QMKonnect"`) at Windows startup and make the
WinRT toast API available for **P1.M4.T1.S2** (which replaces the `MessageBoxW`
modal in `platforms::notify`). The contract offers two approaches; this PRP
picks **Approach A** (extend the existing `windows = "0.52.0"` dep with features).

## Decision: APPROACH A (windows-crate features) — NOT tauri-winrt-notification

Rationale (documented so the choice is defensible / not re-litigated):
1. **Existing pattern.** `Cargo.toml` already pulls `windows = "0.52.0"` with 12
   Win32 features (lines 65-79). `src/platforms/windows.rs` + `src/platforms/mod.rs`
   + `src/autostart.rs` all use it. Adding 3 features to an existing dep is the
   lowest-risk, established pattern; a new external crate is not.
2. **Plan shape implies the installer owns the shortcut.** The plan splits this
   into **T1.S1 (AUMID + deps)** and **T2.S1 (Start Menu .lnk AUMID)**. T2.S1's
   title is *"Add AUMID property to Start Menu shortcut in QMKonnect.iss (or
   runtime fallback)"*. The Inno installer already creates the Start Menu `.lnk`
   (`QMKonnect.iss:93`) — adding `System.AppUserModel.ID` there is the natural
   fix. Approach B's `PowerShell::create_shortcut` would create the `.lnk` at
   *runtime* (shelling out to PowerShell), making T2.S1 redundant/conflicting.
3. **No PowerShell runtime dependency.** `tauri-winrt-notification`'s
   `PowerShell::create_shortcut` invokes PowerShell on first run — a known
   footgun for a background tray daemon. Approach A avoids it entirely.
4. **Scope.** The toast fires at most once per broken `rules.toml` (deduped in
   `notifier.rs`, see research §3). A whole new crate for one low-frequency toast
   is over-engineering.

## Verified API paths + features (windows 0.52)

Confirmed via windows-rs feature-hierarchy docs (zread.ai/microsoft/windows-rs
§2 "The windows Crate"): each feature corresponds to one namespace module;
enabling a child auto-enables parents; `Win32_Foundation` cascades and must NOT
be specified manually.

| API (Rust path) | Feature string | Purpose |
|---|---|---|
| `windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID` | `Win32_UI_Shell` | Process-level AUMID (this task) |
| `windows::UI::Notifications::*` (`ToastNotificationManager`, `ToastNotification`, `ToastNotifier`) | `UI_Notifications` | Toast send (T1.S2) |
| `windows::Data::Xml::Dom::*` (`XmlDocument`) | `Data_Xml_Dom` | Build the toast XML payload (T1.S2) |

`SetCurrentProcessExplicitAppUserModelID` signature (windows-rs 0.52):
```rust
pub unsafe fn SetCurrentProcessExplicitAppUserModelID<P0>(appid: P0) -> windows::core::Result<()>
where P0: Param<PCWSTR>;
```
→ call as `SetCurrentProcessExplicitAppUserModelID(PCWSTR(ptr))`. `PCWSTR` =
`windows::core::PCWSTR` (already imported in mod.rs:156). It is a **pure Win32
shell32 call — no COM init / RoInitialize needed** (unlike the WinRT toast APIs
in S2, which DO need `CoInitializeEx` on the calling thread).

MSRV: windows 0.52's MSRV is well below the project floor of 1.88 → no conflict.

## AUMID identity notes (avoid confusion)

- **AUMID** = `"Mulletware.QMKonnect"` — the conventional `Publisher.App` toast
  identity (per contract). This is what the running process sets (this task) and
  what the toast `ToastNotificationManager` registers under (S2).
- **Inno `AppId`** = `{FAAE1F7A-9DBD-4C2A-B122-A9A73F05D0B3}` — a *different*
  GUID used for installer upgrade tracking. The contract's phrase "matching the
  Inno installer's AppId" is loose wording; the AUMID and the AppId-GUID are
  distinct identities. **Do NOT use the GUID as the AUMID.**
- **Forward ref (T2.S1, NOT this task):** the Start Menu `.lnk`
  (`QMKonnect.iss:93`) must set `System.AppUserModel.ID = "Mulletware.QMKonnect"`
  or toasts will silently not render. That is T2.S1's scope; this task only
  (a) sets the *process* AUMID and (b) exposes the constant as `pub` for T2.S1
  to reference. A toast needs BOTH the process AUMID AND the .lnk advertising it.

## Where set_aumid() is called

`src/main.rs::main()` — immediately after `init_logging()` succeeds, before
`run()`. Reason: the AUMID is **process-global identity** that must precede any
toast on EVERY code path (`run()` has many early-return subcommand branches:
`--reload`, `--config`, `--list-callbacks`, …). Setting it once in `main()`
covers all paths. `init_logging` is kept first (existing comment). The call is
`#[cfg(target_os = "windows")]`-gated in main.rs (same idiom as the `autostart`
module declaration at main.rs:16).

## Validation asymmetry (THE critical gotcha)

The implementing agent runs on a **Linux dev box**. Consequences:
- `cargo build` / `cargo test` on Linux **compile the non-Windows code only**.
  The `windows`-target features and `set_aumid()`/`SetCurrentProcess*…` code are
  `#[cfg(target_os="windows")]`-gated → NOT compiled on Linux.
- **Partial gate that DOES run on Linux:** `cargo build`'s dependency resolver
  validates that the feature names (`Win32_UI_Shell`, `UI_Notifications`,
  `Data_Xml_Dom`) are real features of `windows 0.52.0`. A typo fails the build
  on Linux. ✅
- **The compile-time const assertion** (`const _: () = assert!(!APP_AUMID.is_empty());`)
  references only a plain `&str` constant → it IS compiled on Linux, so it acts
  as a real (cross-platform) gate. ✅
- **NOT validated on Linux:** the actual `SetCurrentProcessExplicitAppUserModelID`
  call site and that the toast API paths resolve. Those are validated on Windows
  per **AGENTS.md Windows dev loop** (`cargo build --release` on the canonical
  path, `taskkill` + run the exe). This PRP marks those gates as DEFERRED and
  documents the manual Windows check.

## Cross-task coordination (no conflicts)

- **P1.M3.T2.S1 (parallel, in-flight):** edits `src/core/notifier.rs` ONLY (its
  PRP pins `git diff --stat` to that one file). This task edits `Cargo.toml`,
  `src/platforms/mod.rs`, `src/main.rs`. **Zero file overlap** → safe to merge in
  either order.
- **P1.M4.T1.S2 (downstream):** consumes the 3 new windows features +
  `pub const APP_AUMID`. It replaces the `MessageBoxW` block in `notify()`'s
  Windows arm (mod.rs:150-166). This task must NOT touch that block.
- **P1.M4.T2.S1 (downstream):** references the documented `APP_AUMID` value in
  `QMKonnect.iss`'s Start Menu shortcut. This task exposes it `pub` + documents
  it; the .lnk edit is T2.S1.

## Files this task touches (exact)
1. `Cargo.toml` — add 3 features to the windows dep (target windows), with comments.
2. `src/platforms/mod.rs` — add `pub const APP_AUMID` + compile-time const check +
   `#[cfg(windows)] pub fn set_aumid()`.
3. `src/main.rs` — gated call to `platforms::set_aumid()` in `main()`.
DOCS: none (Approach A → no `docs/installation.md` change, per contract Mode-A).