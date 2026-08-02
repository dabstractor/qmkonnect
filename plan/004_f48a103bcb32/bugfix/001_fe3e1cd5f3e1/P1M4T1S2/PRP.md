# PRP — P1.M4.T1.S2: Implement WinRT toast replacing MessageBoxW in `platforms::notify`

> **Repo:** QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust menu-bar/tray daemon.
> **Files (3, all already committed at HEAD `17e4f6f`):** `Cargo.toml` (`Win32_System_Com`),
> `src/platforms/mod.rs` (`show_toast` / `build_toast_xml` / `xml_escape` + cfg(windows) test),
> `docs/troubleshooting.md` (toast note). See **STATUS** below.
> **Approach:** A (raw `windows = "0.52.0"` crate) — **inherited from P1.M4.T1.S1**
> (which rejected `tauri-winrt-notification` and added 3 features + `APP_AUMID` +
> `set_aumid()`). This task CONSUMES S1's outputs and lands the actual toast call.
>
> **The defect this closes (delivery half):** bug-hunt **Finding #3** (PRD `h2.1`
> #3) — `platforms::notify` on Windows is a focus-stealing, desktop-modal
> `MessageBoxW` on a *detached thread that leaks until the user clicks OK*, not
> the spec-mandated toast (`spec/UI.md` §2.3, `spec/HOST_RULES.md` §7). S1 set the
> process AUMID + enabled the toast API features; **S2 replaces the modal with a
> real fire-and-forget toast.** A remaining prerequisite — the Start Menu `.lnk`
> advertising the AUMID — is **P1.M4.T2.S1** (Inno installer); until it lands the
> `Show()` call succeeds but the toast is silently not rendered (by design — see
> research §Q7). **S2 is still correct and complete to merge without T2.S1.**

---

## STATUS — read first (the implementation is already committed)

**Both P1.M4.T1.S1 and S2 are already committed.** `git log --oneline -2`:

```
17e4f6f Replace Windows modal with WinRT toast          ← THIS task (S2): DONE + committed
8eb03ac Initialize Windows toast identity and dependencies  ← S1: DONE + committed
```

The committed S2 implementation matches this PRP's design exactly in intent:
`show_toast` (short-lived thread → `CoInitializeEx(STA)` → build/load XML →
`ToastNotification::CreateToastNotification` → `ToastNotificationManager::CreateToastNotifierWithId(&APP_AUMID)`
→ `ToastNotifier::Show`, with `log::warn!` on failure), `build_toast_xml` (`ToastText02`),
`xml_escape`, the `#[cfg(all(test, target_os = "windows"))]` XML-parse test, and the
`docs/troubleshooting.md` note. `Win32_System_Com` is at `Cargo.toml:98`.

**Therefore this PRP is the authoritative design record + verification / regression
checklist for the committed code.** Tasks 1–4 read as *"verify the committed state is as
specified (and restore it if a future edit regressed it)"*; the implementing agent's job is
**confirmation + running the validation gates**, not a from-scratch write. The orchestrator's
`plan_status` (S2 = "Researching") is stale relative to git — **treat git as the source of truth.**

---

## Goal

**Feature Goal**: Replace the Windows arm of `platforms::notify` (the detached
`std::thread::spawn(MessageBoxW(…))` block in `src/platforms/mod.rs`) with a real
WinRT toast built from a `ToastText02` XML payload and fired via
`ToastNotificationManager::CreateToastNotifierWithId(&APP_AUMID)` + `Show()`.
Eliminate the modal dialog, the focus-stealing, and the thread-that-blocks-until-
click. Keep the function signature `pub fn notify(title: &str, body: &str)` and
the caller's dedup (`notifier.rs` `RULES_INVALID_NOTIFIED`) byte-for-byte unchanged.

**Deliverable** (exactly three files):
1. **`Cargo.toml`** — add **`Win32_System_Com`** to the `windows` dep's
   `features` (the *only* feature S2 adds; S1 already added `Win32_UI_Shell`,
   `UI_Notifications`, `Data_Xml_Dom`). Needed for `CoInitializeEx`/`CoUninitialize`.
2. **`src/platforms/mod.rs`** — replace the `#[cfg(target_os = "windows")]` arm
   of `notify()` (the `MessageBoxW` block) with a call to a new private
   `#[cfg(target_os = "windows")] fn show_toast(title, body)`. Add helpers
   `build_toast_xml()` + `xml_escape()` (cfg-windows private fns, mirroring the
   existing `osa_string` precedent). Add one `#[cfg(all(test, target_os =
   "windows"))]` unit test verifying the built XML parses via
   `XmlDocument::LoadXml` (no `Show`, no shortcut needed).
3. **`docs/troubleshooting.md`** — in the `### \`rules.toml\` parse error` section,
   add a one-paragraph note that the runtime notification is a **toast on Windows
   (auto-dismissing, appears in Action Center)** / `notify-send` on Linux /
   Notification Center alert on macOS (Mode-A docs, per contract).

**Success Definition**:
- On the **Linux dev box**: `cargo build` succeeds (the resolver validates
  `Win32_System_Com` is a real windows-0.52 feature); `cargo test --bin qmkonnect
  -- --test-threads=1` is green with the test count **unchanged** (the new test is
  `#[cfg(windows)]` and is not compiled/run on Linux). The work is **committed at HEAD
  `17e4f6f`**, so `git show 17e4f6f --stat` touches exactly those three source files
  (+ this PRP/research) and the working-tree `git diff` is empty.
- On **Windows** (deferred to the AGENTS.md Windows dev loop — the implementing
  agent runs on Linux and cannot compile the cfg-gated toast code): `cargo build
  --release` on the canonical path resolves `CoInitializeEx`,
  `XmlDocument::LoadXml`, `ToastNotification::CreateToastNotification`,
  `ToastNotificationManager::CreateToastNotifierWithId`, `ToastNotifier::Show`;
  the `cfg(windows)` unit test passes; and with a Start Menu `.lnk` advertising
  `APP_AUMID` (P1.M4.T2.S1) a real toast renders on a broken `rules.toml`.

## User Persona (if applicable)

**Target User**: the Windows end user whose `rules.toml` fails to parse.
**Use Case**: the user edits `rules.toml` and saves a syntax error. On the next
window focus change, `host_context_for_window` re-parses it, the parse fails, the
dedup fires one notification, and the user is informed — *without* a modal dialog
grabbing focus and without a leaked background thread.
**User Journey** (today → after S2):
- Today: a modal `MessageBoxW` pops up over whatever app has focus, must be
  clicked away, and the spawned thread lives until dismissed.
- After S2: a toast slides in from the Action Center (or, pre-T2.S1, nothing
  visible yet — but no modal, no thread leak, no focus steal), auto-dismisses
  after ~7s, and is reviewable later in Action Center.
**Pain Points Addressed**: focus stealing (Finding #3: "desktop-modal, steals
focus, needs a click"), thread leak (Finding #3: "leaks a thread until
dismissed"), spec violation (`spec/UI.md` §2.3 mandates a toast).

## Why

- **Spec compliance.** `spec/UI.md` §2.3 and `spec/HOST_RULES.md` §7 both
  explicitly mandate a **toast** on Windows for the rules-invalid notification.
  The current `MessageBoxW` is a documented stop-gap (the author's own comment at
  `mod.rs:190`: "Dep-free stand-in for a WinRT toast"). S1 removed the
  infrastructure blocker (AUMID + API features); S2 removes the stop-gap itself.
- **Eliminates a real defect.** The detached `thread::spawn(MessageBoxW)` (a)
  foreground-activates a top-level window over the user's work, (b) runs its own
  modal `GetMessage` loop that only returns on `IDOK`/close, and (c) because the
  `JoinHandle` is dropped (detached), the OS thread stays alive until the user
  clicks. A toast is none of these: `Show()` returns immediately, the toast
  auto-dismisses, and the worker thread (if any) exits in <1 ms.
- **Completes the S1→S2→T2.S1 chain.** S1 set the process AUMID. S2 (this task)
  emits the toast. T2.S1 advertises the AUMID on the Start Menu `.lnk` so the
  toast actually renders. S2 is independently mergeable: `Show()` returns `Ok`
  even without the `.lnk` (research §Q7) — it just renders nothing until T2.S1.

## What

### Approach: A (raw `windows` crate) — NOT revisited

Inherited from S1 (which rejected `tauri-winrt-notification` for 4 documented
reasons: existing-dep pattern, installer-owns-the-shortcut split, no PowerShell
runtime dep, scope). S1 already enabled `UI_Notifications` + `Data_Xml_Dom` +
`Win32_UI_Shell`. **Do not introduce `tauri-winrt-notification`, `notify-rust`,
or any new crate.** S2 adds exactly one more feature (`Win32_System_Com`).

### Design decision: keep a *short-lived* thread for COM-init isolation

The contract said *"remove the `std::thread::spawn` wrapper (toasts are
inherently non-blocking)."* That instruction was written before the COM-init
requirement was known. Research (`research/winrs_toast_api.md` §Q2) establishes
that **every WinRT toast call requires the calling thread to be COM-initialized**
(`CoInitializeEx`), unlike S1's pure-Win32 `set_aumid()` which needs none.
Therefore:

- **DO keep a `std::thread::spawn` wrapper** — but a *short-lived* one. The toast
  worker does: `CoInitializeEx(STA)` → build XML → `LoadXml` →
  `CreateToastNotification` → `CreateToastNotifierWithId` → `Show` (all fast; `Show`
  is non-blocking) → drop COM objects → `CoUninitialize` → **thread exits in <1 ms**.
- **This is NOT the Finding #3 thread-leak.** That defect was a thread that
  *blocked* on a modal dialog until the user clicked. The toast thread blocks on
  nothing user-facing; it exits the instant `Show` returns.
- **Why not call it inline on the calling thread?** `notify()` is reached from
  `host_context_for_window` on the window-event-processing thread, which may
  already hold a COM apartment (e.g. an MTA initialized by the event loop / HID
  stack). Calling `CoInitializeEx(None, COINIT_APARTMENTTHREADED)` there returns
  `RPC_E_CHANGED_MODE` and the subsequent WinRT calls can fail — a subtle,
  hard-to-debug break. A fresh thread guarantees a clean STA apartment. **This is
  the standard pattern used by tray/notification libraries** (research §Q2,
  "short-lived thread pattern").
- The COM-init + Show happen entirely inside the spawned closure; `notify()`
  itself returns immediately (the `JoinHandle` is detached, exactly as before).

> **Net effect vs. the old code:** same `notify()` control flow (spawn + detach),
> but the body is a fast fire-and-forget toast instead of a blocking modal. No
> focus steal, no click required, no long-lived thread. The contract's *intent*
> ("non-blocking, no modal") is fully satisfied; only the literal "no spawn
> wrapper" wording is deviated from, for the COM-safety reason above.

### DOCS (Mode A): one paragraph in `docs/troubleshooting.md`
Per the contract: update the section that documents the `rules.toml`-invalid
notification to reflect toast behavior. See Implementation Task 4 for the exact
anchor. **No** `docs/installation.md` change (Approach A → Mode A → none).

### Success Criteria
- [ ] `Cargo.toml`: `Win32_System_Com` added to the `windows` features array (Windows target), alongside S1's 3 features; no feature removed, no version bump, no new dep.
- [ ] `src/platforms/mod.rs`: the `MessageBoxW` Windows arm of `notify()` is **gone**; replaced by `show_toast(title, body)`.
- [ ] `src/platforms/mod.rs`: new `#[cfg(target_os = "windows")] fn show_toast(title: &str, body: &str)` that spawns a short-lived thread → `CoInitializeEx` → build+load XML → `CreateToastNotification` → `CreateToastNotifierWithId(&APP_AUMID)` → `Show`, swallowing all errors (`let _ = …`).
- [ ] `src/platforms/mod.rs`: new `#[cfg(target_os = "windows")] fn build_toast_xml(title, body) -> String` and `#[cfg(target_os = "windows")] fn xml_escape(s) -> String` helpers (XML-escape `& < > " '`).
- [ ] `src/platforms/mod.rs`: a `#[cfg(all(test, target_os = "windows"))] mod toast_tests` with a test that builds XML (incl. a body with `& " <`) and asserts `XmlDocument::LoadXml` parses it (no `Show`, no shortcut).
- [ ] The Linux and macOS arms of `notify()` are **byte-for-byte unchanged**.
- [ ] `notify()`'s signature `pub fn notify(title: &str, body: &str)` and its doc comment's contract (best-effort, fire-and-forget, caller dedupes) are **unchanged**.
- [ ] The caller (`notifier.rs::host_context_for_window`) and `RULES_INVALID_NOTIFIED` dedup are **untouched**.
- [ ] `docs/troubleshooting.md`: the rules.toml-parse-error section notes Windows shows a toast (auto-dismissing, Action Center).
- [ ] Linux: `cargo build` succeeds; `cargo test --bin qmkonnect -- --test-threads=1` green, test count unchanged; `git show 17e4f6f --stat` = the 3 source files (work committed; working-tree `git diff` empty).

## All Needed Context

### Context Completeness Check
_Pass._ An agent with no prior knowledge can implement all three edits from: the
exact verbatim `MessageBoxW` arm to replace (quoted in Task 2), the verified
windows-0.52 API sequence + signatures (research §Q1, with full code recipe), the
COM-init rationale (§Q2), the `osa_string` precedent for a cfg-gated private fn,
the exact docs anchor (Task 4), and the AGENTS.md gates. The only judgment call
(the short-lived-thread deviation) is already decided and justified above. The
validation asymmetry (Linux agent can't compile the cfg-gated toast code) is
documented as a deferred gate.

### Documentation & References

```yaml
# MUST READ — the bug-hunt research that pinned the defect, the single trigger, and the dedup
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/architecture/windows_notify_research.md
  why: "§1 pins the EXACT code to change (mod.rs notify() Windows arm) and confirms every sub-claim
        of Finding #3. §2/§3 confirm the SINGLE caller (host_context_for_window on a parse failure)
        and that the dedup is correct — DO NOT change either. §4 quotes the spec (UI.md §2.3 +
        HOST_RULES.md §7 → toast). §5 lists the missing features. §6 shows Linux/macOS are correct
        and only Windows deviates."
  section: "all (§1–§6); especially §1, §3, §5"
  critical: "single file to change = src/platforms/mod.rs notify() Windows arm. single trigger =
        rules.toml parse error. dedup + trigger are correct — leave alone. Linux/macOS arms unchanged."

# MUST READ — the verified windows-0.52 toast API recipe (THIS is the implementation playbook)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S2/research/winrs_toast_api.md
  why: "§Q1 gives the EXACT API call sequence + signatures for windows 0.52 (PascalCase method names:
        XmlDocument::new(), LoadXml(&HSTRING), ToastNotification::CreateToastNotification(&XmlDocument),
        ToastNotificationManager::CreateToastNotifierWithId(&HSTRING), ToastNotifier::Show(&toast)).
        §Q2 mandates CoInitializeEx on the calling thread (feature Win32_System_Com) and recommends the
        short-lived-thread pattern. §Q3 = HSTRING::from(&str). §Q4 = the ToastText02 template + the
        XML-escape requirement (the body is a TOML parse error with quotes/ampersands). §Q7 = Show()
        returns Ok even without the .lnk (so S2 is mergeable before T2.S1). A full compilable recipe
        is at the end of the doc."
  section: "§Q1–Q7 + 'Complete Compilable Recipe'"
  critical: "method names are PascalCase (LoadXml, Show) — snake_case variants do NOT exist in 0.52 and
        will fail to compile. CreateToastNotifierWithId (NOT CreateToastNotifier) takes the explicit
        AUMID. ToastNotification has NO ::new() — only the CreateToastNotification factory. WinRT toast
        calls REQUIRE CoInitializeEx; set_aumid() did not because it is pure Win32."

# MUST READ — the S1 PRP (the contract S2 consumes; assume S1 landed EXACTLY as specified)
- docfile: plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S1/PRP.md
  why: "defines the outputs S2 builds on: pub const APP_AUMID = \"Mulletware.QMKonnect\", set_aumid()
        (called in main() before run()), and the 3 windows features (Win32_UI_Shell, UI_Notifications,
        Data_Xml_Dom). Confirms S1 explicitly did NOT touch notify()'s MessageBoxW arm (S2's job) and
        did NOT add Win32_System_Com (S2 must add it). Confirms AUMID ≠ Inno AppId GUID."
  section: "Goal, What (Approach selection), Success Criteria, Anti-Patterns"
  critical: "S1 is IMPLEMENTED (verified: APP_AUMID at mod.rs:134, set_aumid() at :149, 3 features in
        Cargo.toml). S2 references APP_AUMID (already pub) and the 3 features. S2 ADDS Win32_System_Com."

# MUST READ — the file being edited (contains APP_AUMID, set_aumid, notify(), osa_string, the test mod)
- file: src/platforms/mod.rs
  why: "the notify() Windows arm (MessageBoxW block, lines ~187–206 — exact text in Task 2) is what S2
        replaces. osa_string (cfg-macos private fn) is the PRECEDENT for a cfg-gated private helper
        (show_toast/build_toast_xml/xml_escape follow it). APP_AUMID (line 134) is already pub const.
        The existing #[cfg(test)] mod tests (near the end) shows where to add the new cfg(windows) test mod."
  pattern: "platform-gated private fn: `#[cfg(target_os = \"macos\")] fn osa_string(s: &str) -> String { … }`.
        cfg-windows private fns (show_toast/build_toast_xml/xml_escape) follow the same shape. The new
        test module is `#[cfg(all(test, target_os = \"windows\"))]` so it compiles ONLY on Windows."
  gotcha: "do NOT touch the Linux/macOS arms of notify(), the notify() signature, or the doc comment.
        do NOT move/touch APP_AUMID/set_aumid (S1's). Keep the spawn+detach control flow; only swap the body."

# REFERENCE — the caller (do NOT change; read-only to confirm the contract is honored)
- file: src/core/notifier.rs
  why: "host_context_for_window (the ONLY caller of platforms::notify) at ~line 1086, and the
        RULES_INVALID_NOTIFIED dedup at ~line 264. S2 does not edit this file. Read it to confirm: the
        notification title is \"QMKonnect: rules.toml invalid\" and body is `format!(\"{e}\")` (a TOML
        parse error string that may contain & \" < > — hence the xml_escape requirement)."
  pattern: "`if !RULES_INVALID_NOTIFIED.swap(true, Ordering::SeqCst) { crate::platforms::notify(\"QMKonnect: rules.toml invalid\", &format!(\"{e}\")); }`"
  gotcha: "the body is an ARBITRARY error string. It WILL contain characters that break XML if not
        escaped. xml_escape is mandatory, not optional."

# REFERENCE — the Cargo.toml windows features (S1 added 3; S2 adds Win32_System_Com)
- file: Cargo.toml
  why: "the [target.'cfg(target_os = \"windows\")'.dependencies] windows dep (lines ~66–93). S1 added
        Win32_UI_Shell / UI_Notifications / Data_Xml_Dom with a comment block. S2 appends Win32_System_Com
        with a one-line comment (for CoInitializeEx). No version bump, no new dep entry."
  pattern: "append `\"Win32_System_Com\",` to the existing features array, next to S1's three, with a
        `# WinRT toast: CoInitializeEx/CoUninitialize (P1.M4.T1.S2) — WinRT toast calls require a COM apartment.` comment."
  gotcha: "Win32_System_Com is NOT auto-enabled by UI_Notifications/Data_Xml_Dom; it must be listed
        explicitly. Verify via the Level-4 feature-resolution check."

# EXTERNAL — canonical windows-0.52 toast rustdoc (confirms API names/signatures in Q1)
- url: https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotificationManager.html
  why: "confirms CreateToastNotifierWithId(applicationid: &HSTRING) -> Result<ToastNotifier> is the
        static that takes the AUMID. (NOT CreateToastNotifier, which is the parameterless overload.)"
  critical: "version-pinned to 0.52.0 in the URL — do NOT consult the 'latest' docs (API surface shifts)."

# EXTERNAL — CoInitializeEx (the new COM-init call S2 introduces)
- url: https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Win32/System/Com/fn.CoInitializeEx.html
  why: "confirms signature CoInitializeEx(pvreserved: Option<*const c_void>, dwcoinit: COINIT) -> HRESULT
        and that pvreserved must be None. Feature = Win32_System_Com. S_FALSE (already-initialized) is a success."
  critical: "call on the SAME thread as the WinRT calls (the spawned worker). COINIT_APARTMENTTHREADED (STA)
        is the safe choice for toast UI components."

# EXTERNAL — ToastText02 template + the Start-Menu-shortcut requirement (why T2.S1 is still needed)
- url: https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl
  why: "confirms (a) the ToastText02 two-line template, (b) that Show() does NOT error on a missing
        shortcut (the toast is silently suppressed) — which is why S2 is mergeable before T2.S1, and
        (c) the Start Menu .lnk must set System.AppUserModel.ID = the AUMID (T2.S1's job)."
  critical: "do NOT expect a visible toast after S2 alone if the .lnk is absent — Show() returns Ok and
        nothing renders. That is correct; T2.S1 completes the chain."
```

### Current Codebase tree (relevant slice, post-S1)

```bash
# run from /home/dustin/projects/qmkonnect
Cargo.toml
  - :66-93  [target.'cfg(target_os="windows")'.dependencies] windows features
            S1 ADDED: Win32_UI_Shell, UI_Notifications, Data_Xml_Dom  ← S2 appends Win32_System_Com here
src/platforms/mod.rs
  - :128-164 pub const APP_AUMID = "Mulletware.QMKonnect"; + const-check + #[cfg(windows)] pub fn set_aumid()  ← S1 (DO NOT TOUCH)
  - :167-232 pub fn notify(title, body) {
                :169-176 #[cfg(linux)]   notify-send arm        ← DO NOT TOUCH
                :178-186 #[cfg(macos)]   osascript arm          ← DO NOT TOUCH
                :187-206 #[cfg(windows)] MessageBoxW arm        ← REPLACE THIS (Task 2)
                :207-210 #[cfg(not(any…))] no-op arm            ← DO NOT TOUCH
              }
  - :213-216 #[cfg(macos)] fn osa_string(s) -> String          ← PRECEDENT for cfg-gated private fn
  - :255-…   #[cfg(test)] mod tests { … }                      ← add #[cfg(all(test,windows))] mod toast_tests nearby
src/core/notifier.rs
  - :264-267 static RULES_INVALID_NOTIFIED                       ← DO NOT TOUCH (dedup)
  - :~1086   host_context_for_window → platforms::notify(...)    ← the ONLY caller (DO NOT TOUCH)
docs/troubleshooting.md
  - :507-530 ### `rules.toml` parse error                        ← add a platform-note paragraph (Task 4)
```

### Desired Codebase tree with files added/changed

```bash
Cargo.toml                      # +1 feature: "Win32_System_Com" (for CoInitializeEx)
src/platforms/mod.rs            # notify() Windows arm: MessageBoxW → show_toast(title, body)
                                # +#[cfg(windows)] fn show_toast(title, body)
                                # +#[cfg(windows)] fn build_toast_xml(title, body) -> String
                                # +#[cfg(windows)] fn xml_escape(s) -> String
                                # +#[cfg(all(test, target_os="windows"))] mod toast_tests (1 test)
docs/troubleshooting.md         # +1 paragraph in "rules.toml parse error" section (toast on Windows)
# (no new files; no new deps; no version bumps)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (COM init is REQUIRED for WinRT toast, unlike S1's set_aumid): every WinRT call
//   (XmlDocument::new, LoadXml, CreateToastNotification, CreateToastNotifierWithId, Show) must run
//   on a thread that called CoInitializeEx. S1's set_aumid() is pure Win32 (shell32) and needs NO
//   COM — do not assume the toast is the same. Feature Win32_System_Com (THIS task adds it) is required.

// CRITICAL (why we KEEP a short-lived thread despite the contract saying "remove spawn"): the calling
//   thread (window-event processing) may already hold a COM apartment (MTA) from the event loop / HID
//   stack. Calling CoInitializeEx(None, STA) there returns RPC_E_CHANGED_MODE and WinRT calls can fail.
//   A fresh worker thread gets a clean STA. The worker exits in <1 ms (Show is non-blocking) → NOT the
//   Finding #3 "leaks until click" defect. See the "Design decision" section above.

// CRITICAL (method names are PascalCase in windows 0.52): LoadXml (NOT load_xml), Show (NOT show),
//   CreateToastNotification (NOT new/Create), CreateToastNotifierWithId (NOT CreateToastNotifier — that
//   is the no-AUMID overload). Blog/StackOverflow examples targeting windows-rs <0.40 use snake_case
//   and WILL NOT compile on 0.52. Trust research/winrs_toast_api.md §Q1, not random examples.

// CRITICAL (HSTRING ≠ PCWSTR): the toast APIs take &windows::core::HSTRING (built via
//   HSTRING::from("…") or HSTRING::from(string.as_str())). S1's set_aumid used PCWSTR + manual UTF-16
//   because SetCurrentProcessExplicitAppUserModelID is a Win32 fn. Do NOT reuse the PCWSTR/Vec<u16>
//   pattern for the toast — it takes HSTRING. (Repo has ZERO existing HSTRING usage — this is the first.)

// CRITICAL (XML-escape the body): the body is a TOML parse error string (notifier.rs formats `{e}`).
//   It WILL contain & " < > in real failures. Unescaped, LoadXml fails (XML parse error) and the toast
//   silently does not fire. xml_escape MUST escape & < > " ' (escape & FIRST to avoid double-escaping).

// CRITICAL (toast won't render until T2.S1): CreateToastNotifierWithId + Show return Ok(()) even when
//   no Start Menu .lnk advertises APP_AUMID — the toast is silently suppressed. S2 is CORRECT and
//   mergeable without T2.S1; the visible toast is T2.S1's deliverable. Do not add runtime .lnk creation
//   (that is T2.S1's Inno-installer scope; S1 rejected Approach B partly for this reason).

// GOTCHA (validation asymmetry): the implementing agent runs on LINUX. cargo build/test there compile
//   ONLY non-Windows code. The toast code (show_toast, build_toast_xml, xml_escape, the cfg(windows)
//   test) is NOT compiled on Linux. Gates that DO run on Linux: (1) cargo resolver validates
//   Win32_System_Com is a real windows-0.52 feature; (2) the Linux/macOS arms + notify() signature
//   unchanged → existing tests stay green at the same count. The actual toast compile + the cfg(windows)
//   test run on WINDOWS (AGENTS.md loop) — mark DEFERRED.

// GOTCHA (APP_AUMID is already pub const from S1): show_toast references APP_AUMID directly (no
//   redeclaration). Do NOT redefine it. Do NOT change it. It is "Mulletware.QMKonnect" (NOT the Inno
//   AppId GUID {FAAE1F7A-…}).

// GOTCHA (do not touch notify()'s other arms or signature): the contract requires the signature
//   `pub fn notify(title: &str, body: &str)` and the Linux/macOS arms to be unchanged. Only the
//   #[cfg(target_os="windows")] arm's BODY changes (it now calls show_toast(title, body)).

// GOTCHA (dedup + trigger are correct and out of scope): RULES_INVALID_NOTIFIED (notifier.rs:264) and
//   host_context_for_window (notifier.rs:~1086) are NOT edited by S2. The "QMKonnect: rules.toml
//   invalid" title string is the caller's choice; S2 just renders whatever title/body it receives.

// GOTCHA (single-threaded tests): AGENTS.md mandates `--test-threads=1` (shared global debouncer/mock
//   state). The new cfg(windows) test inherits this when run on Windows.
```

## Implementation Blueprint

### Data models and structure
None new. No structs, enums, config keys, or env vars. Three cfg-windows private
free functions (`show_toast`, `build_toast_xml`, `xml_escape`) + one cfg-windows
test module. All consume the already-present `pub const APP_AUMID` from S1.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: VERIFY/EDIT Cargo.toml — `Win32_System_Com` feature  (ALREADY COMMITTED at HEAD)
  In `[target.'cfg(target_os = "windows")'.dependencies]`, the `windows` dep's `features` array
  must contain `Win32_System_Com` (for `CoInitializeEx`/`CoUninitialize`). **In the current tree
  it is ALREADY PRESENT** — `Cargo.toml:98`, committed in `17e4f6f` (the comment block at :95-98
  explains it). So this task is normally a no-op VERIFY:

  ```bash
  grep -n 'Win32_System_Com' Cargo.toml   # Expected: the feature line at :98 + comment refs
  ```

  If (only if) a future edit removed it, restore it by appending to the `features` array,
  right after S1's three toast features (`Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom`):

  ```toml
      # WinRT toast COM init (P1.M4.T1.S2): CoInitializeEx/CoUninitialize — WinRT
      # toast calls require the calling thread to hold a COM apartment (unlike
      # set_aumid above, which is pure Win32 and needs none). Sibling to the three
      # features directly above.
      "Win32_System_Com",
  ```

  - PRESERVE: every existing feature (Win32_Foundation … Win32_System_Registry, plus S1's 3).
    Do NOT change the version (`= "0.52.0"`) or add a new dependency entry. `Win32_System_Com`
    is NOT auto-enabled by `UI_Notifications`/`Data_Xml_Dom`; it must be listed explicitly.

Task 2: EDIT src/platforms/mod.rs — replace notify()'s Windows arm + add the toast helpers

  STEP 2a — replace the MessageBoxW arm. Run `sed -n '185,210p' src/platforms/mod.rs` first to
  confirm the exact current text (S1 shifted line numbers; the block below is verbatim from the
  current tree). The block to replace is:

  ```rust
      #[cfg(target_os = "windows")]
      {
          // Dep-free stand-in for a WinRT toast (a true toast needs an
          // AppUserModelID + Start Menu shortcut to render). Non-blocking via a
          // spawned thread; mirrors tray.rs's MessageBoxW idiom.
          let body_w: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
          let title_w: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
          std::thread::spawn(move || {
              use windows::core::PCWSTR;
              use windows::Win32::Foundation::HWND;
              use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
              unsafe {
                  let _ = MessageBoxW(
                      HWND(0),
                      PCWSTR(body_w.as_ptr()),
                      PCWSTR(title_w.as_ptr()),
                      MB_OK | MB_ICONERROR,
                  );
              }
          });
      }
  ```

  Replace that ENTIRE block with (calls the new show_toast helper; keeps the cfg-gate):

  ```rust
      #[cfg(target_os = "windows")]
      {
          // WinRT toast (P1.M4.T1.S2). Fire-and-forget; the caller dedupes and
          // show_toast logs a warn on failure. Replaces the former focus-stealing
          // MessageBoxW modal (bug-hunt Finding #3 / spec UI.md §2.3).
          show_toast(title, body);
      }
  ```

  - PRESERVE the `#[cfg(target_os = "windows")]` attribute line and the surrounding
    Linux/macOS/not(any(…)) arms. Only the BODY of the windows arm changes.

  STEP 2b — add the three cfg-windows private helpers. Place them immediately AFTER notify()
  closes and BEFORE the `osa_string` fn (i.e. between notify()'s closing `}` and the
  `/// Quote a Rust string as an AppleScript …` doc comment), mirroring how osa_string sits as
  a cfg-gated helper right after notify(). Insert EXACTLY this (real newlines, not escapes):

  ```rust
  /// Send a WinRT toast for the "rules.toml invalid" notification (P1.M4.T1.S2).
  ///
  /// Fire-and-forget: logs a `warn!` on failure (same posture as S1's `set_aumid`,
  /// preferable to a silent `let _ = …`). Runs on a SHORT-LIVED worker thread because
  /// every WinRT call needs the calling thread COM-initialized (CoInitializeEx), and the
  /// window-event thread that reaches `notify()` may already hold an incompatible (MTA)
  /// apartment — a fresh STA worker avoids RPC_E_CHANGED_MODE. The worker exits in
  /// <1 ms (`Show` is non-blocking), so this is NOT the former "leaks until click"
  /// defect (bug-hunt Finding #3). The toast actually renders only once a Start Menu
  /// shortcut advertises `APP_AUMID` (P1.M4.T2.S1); until then `Show` returns `Ok(())`
  /// and nothing is visible — by design (research §Q7).
  #[cfg(target_os = "windows")]
  fn show_toast(title: &str, body: &str) {
      // Own the strings for the moved closure (notify's &str don't outlive it).
      let title = title.to_string();
      let body = body.to_string();
      std::thread::spawn(move || {
          use windows::core::HSTRING;
          use windows::Data::Xml::Dom::XmlDocument;
          use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
          use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

          // 1. COM apartment on THIS thread (STA). Required for every WinRT call below;
          //    `let _ =` because S_FALSE (already-init) is benign and RPC_E_CHANGED_MODE
          //    is impossible on a fresh thread. Same thread does all WinRT work + Show.
          unsafe {
              let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
          }

          // 2. Fire-and-forget: build XML → load → wrap → notifier → show. A failure logs
          //    a warn (matches set_aumid); `log` is a windows-target dep and init_logging()
          //    runs before any notify() call. `log::warn!` is fully-qualified so no `use log;`.
          let res = (|| -> windows::core::Result<()> {
              let xml = build_toast_xml(&title, &body);
              let doc = XmlDocument::new()?;
              doc.LoadXml(&HSTRING::from(xml.as_str()))?;
              let toast = ToastNotification::CreateToastNotification(&doc)?;
              let notifier =
                  ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_AUMID))?;
              notifier.Show(&toast)?;
              Ok(())
          })();
          if let Err(e) = res {
              log::warn!("show_toast: toast failed: {e}");
          }

          // 3. Release COM objects (they dropped when the closure above returned) then
          //    uninitialize. Optional (thread exit cleans up anyway) but tidy + matches init.
          unsafe {
              CoUninitialize();
          }
      });
  }

  /// Build the `ToastText02` toast XML (bold title + wrapped body) for `show_toast`.
  /// Factored out so the cfg(windows) unit test can verify escaping + parse-ability
  /// WITHOUT calling `Show` (no shortcut, no popup during `cargo test`).
  #[cfg(target_os = "windows")]
  fn build_toast_xml(title: &str, body: &str) -> String {
      format!(
          "<toast><visual><binding template=\"ToastText02\">\
           <text id=\"1\">{}</text>\
           <text id=\"2\">{}</text>\
           </binding></visual></toast>",
          xml_escape(title),
          xml_escape(body),
      )
  }

  /// Escape a string for XML element content. The toast body is a TOML parse error
  /// that may contain `& " < > '` — unescaped, `LoadXml` rejects it and the toast
  /// silently never fires. Escape `&` first to avoid double-escaping the others.
  #[cfg(target_os = "windows")]
  fn xml_escape(s: &str) -> String {
      s.replace('&', "&amp;")
          .replace('<', "&lt;")
          .replace('>', "&gt;")
          .replace('"', "&quot;")
          .replace('\'', "&apos;")
  }
  ```

  - ERROR POSTURE: `log::warn!` on the rare toast-Result error (preferred over a silent
    `let _ =`; matches set_aumid). `log` is a windows-target dep; `log::warn!` is fully-
    qualified so no `use` is needed inside the closure.
  - NAMING: snake_case free fns (show_toast, build_toast_xml, xml_escape) — matches repo
    style and the osa_string precedent. APP_AUMID is referenced (not redeclared).
  - PRESERVE: osa_string and everything below it; the notify() signature + doc comment; the
    Linux/macOS arms.

Task 3: ADD the cfg(windows) unit test to src/platforms/mod.rs
  Add a new test module. Place it right AFTER the existing `#[cfg(test)] mod tests { … }`
  block (which ends near the bottom of the file), so cfg(windows) test code is grouped with the
  other platform tests. Insert EXACTLY this:

  ```rust
  #[cfg(all(test, target_os = "windows"))]
  mod toast_tests {
      use super::*;

      /// The toast XML must be well-formed AND correctly escaped for the body's special
      /// characters (the body is an arbitrary TOML parse-error string). We verify by loading
      /// it into a real XmlDocument — the same parse show_toast performs — WITHOUT calling
      /// Show (no shortcut needed, no toast pops during `cargo test`).
      ///
      /// NOTE: runs only on Windows (the implementing agent is on Linux, so this is a DEFERRED
      /// gate — see PRP Validation Level 5 / AGENTS.md Windows dev loop).
      #[test]
      fn toast_xml_is_well_formed_and_escapes_special_chars() {
          use windows::core::HSTRING;
          use windows::Data::Xml::Dom::XmlDocument;
          use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

          // XmlDocument::new() is a WinRT activation → needs COM on this test thread.
          unsafe {
              let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
          }

          // A realistic TOML-error body with the chars that break XML if unescaped.
          let xml = build_toast_xml(
              "QMKonnect: rules.toml invalid",
              "expected `=` at line 5 & column 3, found \"<weird>\"",
          );
          let doc = XmlDocument::new().expect("XmlDocument::new");
          doc.LoadXml(&HSTRING::from(xml.as_str()))
              .expect("toast XML must parse after xml_escape");
          // If LoadXml returned Ok, the XML is well-formed and escaping is correct.
      }
  }
  ```

  - COVERAGE: the XML-construction + escaping logic (the only non-trivial logic in this task).
    The COM-init / Show path is pure API glue that cannot be meaningfully unit-tested without a
    registered shortcut, so it is intentionally not tested here.
  - PLACEMENT: after the existing `#[cfg(test)] mod tests`. The `use super::*;` brings
    build_toast_xml into scope (it is a module-level private fn; super::* sees it).
  - GOTCHA: XmlDocument::new() inside the test ALSO needs CoInitializeEx on the test thread —
    include it (cargo test may run the test on a worker thread with no apartment).

Task 4: EDIT docs/troubleshooting.md — add the toast-behavior note (Mode A)
  In the "### `rules.toml` parse error" section, append a paragraph describing the runtime
  notification's appearance per platform. Locate the section's last paragraph (ending "… See the
  [Configuration Guide]({{ site.baseurl }}/configuration) for the full field table.") and insert
  AFTER it (before the next "### Device shows connected…" heading):

  ```markdown
  At runtime, when `rules.toml` fails to parse during a window focus change, QMKonnect shows a
  **one-time desktop notification** (the app dedupes — at most one per broken state) and then
  falls back to string-only mode. On **Windows** this is a **toast** that auto-dismisses after a
  few seconds and lands in Action Center (it is no longer a modal dialog you must click away);
  Linux uses `notify-send` and macOS uses a Notification Center alert. (On Windows the toast
  requires the installed Start Menu shortcut to render — if you launched a dev build directly the
  notification may be silent, but the `--validate-rules` error above is always printed.)
  ```

  - ANCHOR: the unique tail of the parse-error section is the sentence ending "… for the full
    field table." Find it with `grep -n 'full field table' docs/troubleshooting.md`. Insert the
    new paragraph immediately after that line.
  - TONE/SCOPE: one paragraph, user-facing, factual. No internal line numbers, no PRP/task IDs.
    Matches the surrounding prose (Jekyll `{{ site.baseurl }}` links not needed here).

Task 5: VALIDATE (no edits)
  - cargo build                                  # Linux: resolver validates Win32_System_Com.
  - cargo test --bin qmkonnect -- --test-threads=1   # Linux: green, test count UNCHANGED
                                                  #   (the new test is cfg(windows) → not run here).
  - git show 17e4f6f --stat                      # the commit touched exactly 3 source files (committed); working-tree git diff is empty.
  - (DEFERRED to Windows, AGENTS.md loop — see Validation Level 5) cargo build --release on
    the canonical path + cargo test run the cfg(windows) test + manual toast render check.

Task 6: NEVER do these (out of scope / forbidden)
  - DO NOT change notify()'s signature `pub fn notify(title: &str, body: &str)` or its doc comment.
  - DO NOT touch the Linux (`notify-send`) or macOS (`osascript`) arms of notify().
  - DO NOT touch the `#[cfg(not(any(…)))]` no-op arm.
  - DO NOT edit src/core/notifier.rs (the caller, RULES_INVALID_NOTIFIED, host_context_for_window).
  - DO NOT touch APP_AUMID / set_aumid (S1's output) — reference APP_AUMID as-is.
  - DO NOT remove the S1 features (Win32_UI_Shell/UI_Notifications/Data_Xml_Dom) or re-add them.
  - DO NOT introduce tauri-winrt-notification, notify-rust, or any new crate.
  - DO NOT create a runtime Start Menu .lnk (that is P1.M4.T2.S1, Inno installer).
  - DO NOT use the Inno AppId GUID as the AUMID; APP_AUMID is "Mulletware.QMKonnect".
  - DO NOT call CoInitializeEx on the window-event thread inline (RPC_E_CHANGED_MODE risk); use the
    short-lived worker thread. (See the Design Decision section.)
  - DO NOT use snake_case WinRT method names (load_xml/show) — they do not exist in windows 0.52.
  - DO NOT pass PCWSTR to the toast APIs — they take &HSTRING (HSTRING::from).
  - DO NOT edit docs/installation.md (Approach A → Mode A → no change there).
  - DO NOT edit PRD.md, tasks.json, prd_snapshot.md, or .gitignore.
```

### Implementation Patterns & Key Details
```rust
// PATTERN (cfg-gated private fn — mirrors osa_string at mod.rs:213):
//   a platform-specific helper that notify() dispatches to. show_toast/build_toast_xml/xml_escape
//   are #[cfg(target_os="windows")] private fns sitting right after notify(), exactly where
//   osa_string (#[cfg(target_os="macos")]) already sits.

// PATTERN (windows-rs WinRT toast, windows 0.52 — PascalCase, &HSTRING, COM-init required):
#[cfg(target_os = "windows")]
fn show_toast(title: &str, body: &str) {
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{ToastNotification, ToastNotificationManager};
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
        unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED); }   // STA on this thread
        let res = (|| -> windows::core::Result<()> {
            let xml = build_toast_xml(&title, &body);
            let doc = XmlDocument::new()?;
            doc.LoadXml(&HSTRING::from(xml.as_str()))?;
            let toast = ToastNotification::CreateToastNotification(&doc)?;
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&HSTRING::from(APP_AUMID))?;
            notifier.Show(&toast)?;
            Ok(())
        })();
        if let Err(e) = res { log::warn!("show_toast: toast failed: {e}"); }   // matches set_aumid posture
        unsafe { CoUninitialize(); }
    });
}

// PATTERN (XML escape — mandatory because body is an arbitrary TOML error string):
#[cfg(target_os = "windows")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
     .replace('"', "&quot;").replace('\'', "&apos;")   // & FIRST
}

// WHY log::warn! (not silent `let _ =`): set_aumid (S1) uses log::warn! on failure and log is a
//   windows-target dep with init_logging() guaranteed to have run before any notify() call. A single
//   warn line on a toast failure is strictly better than silent swallow for debugging, with no cost.

// WHY the worker thread (despite contract's "remove spawn"): COM-init isolation — see Design Decision.

// WHY APP_AUMID not a literal: single source of truth (S1's pub const); the toast notifier and the
//   future .lnk (T2.S1) must use the SAME string or the toast is silently dropped.
```

### Integration Points
```yaml
CARGO:
  - add to: Cargo.toml → [target.'cfg(target_os = "windows")'.dependencies] → windows.features
  - pattern: append "Win32_System_Com" (1 feature; no version bump, no new dep)
IMPORTS (inside show_toast's spawned closure — local `use`, repo idiom from windows.rs:62):
  - windows::core::HSTRING
  - windows::Data::Xml::Dom::XmlDocument
  - windows::UI::Notifications::{ToastNotification, ToastNotificationManager}
  - windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED}
LOGGING:
  - show_toast logs log::warn! on the (rare) toast-Result error (log is a windows-target dep; init runs first)
PUBLIC API:
  - notify()'s signature UNCHANGED (pub fn notify(title: &str, body: &str))
  - new #[cfg(windows)] PRIVATE fns: show_toast, build_toast_xml, xml_escape (not pub; test reaches via super::*)
CONSUMES (from S1, already landed):
  - pub const APP_AUMID ("Mulletware.QMKonnect") — referenced verbatim, not redeclared
  - windows features Win32_UI_Shell / UI_Notifications / Data_Xml_Dom — already enabled
PARALLEL / SIBLING (no conflicts):
  - P1.M4.T1.S1 (preceding, DONE): edited Cargo.toml + mod.rs (APP_AUMID/set_aumid) + main.rs. S2 adds 1
    more Cargo.toml feature and edits a DIFFERENT region of mod.rs (notify()'s windows arm). Merge clean.
  - P1.M4.T2.S1 (downstream): adds the Start Menu .lnk AUMID in QMKonnect.iss. S2 does NOT touch the .iss.
  - P1.M3.T2.S1 (parallel): edits src/core/notifier.rs ONLY. S2 does not touch notifier.rs. Zero overlap.
PLATFORM VALIDATION:
  - Linux dev box: cargo build (resolver checks Win32_System_Com) + cargo test (arms unchanged → green,
    count unchanged). Cannot compile the cfg-windows toast code.
  - Windows: deferred to AGENTS.md Windows dev loop (cargo build --release + cargo test + manual render).
```

## Validation Loop

> Toolchain: Rust (`cargo`). Tests MUST run single-threaded (AGENTS.md — shared
> global debouncer/mock state). **The implementing agent runs on Linux; the
> `#[cfg(target_os="windows")]` toast code (show_toast/build_toast_xml/xml_escape
> + the cfg(windows) test) is NOT compiled there.** See per-level notes. The
> Windows build + the cfg(windows) test are DEFERRED gates (AGENTS.md loop).

### Level 1: Syntax & Style (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo build
# Expected: compiles. The cargo RESOLVER validates that Win32_System_Com is a real windows-0.52
#   feature (a typo fails here even on Linux). The windows-target toast code is cfg-gated out and
#   not compiled; the Linux/macOS arms + notify() signature are unchanged.
# If "unknown feature `Win32_System_Com`" → fix the feature name (case/underscore-sensitive).
# If "cannot find function `show_toast`" on Linux → you forgot to #[cfg(target_os="windows")]-gate
#   it AND its call site is not gated — the notify() windows arm already is, so show_toast being
#   cfg-windows-only is correct (it's only referenced from inside that arm).
```

### Level 2: Unit Tests / Regression (Linux — runs)
```bash
cd /home/dustin/projects/qmkonnect
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL pass, test count UNCHANGED from baseline. The new toast_xml_… test is
#   #[cfg(all(test, target_os="windows"))] → not compiled on Linux → does not affect the count.
# If a previously-passing test now fails → you accidentally changed the Linux/macOS arm, the
#   notify() signature, or the not(any(…)) no-op arm. Revert and re-apply narrowly.
```

### Level 3: Scope / Build Hygiene (Linux — runs)
> The work is **committed at HEAD `17e4f6f`**, so `git diff` vs the working tree is empty for
> source (only this `PRP.md` is modified). Verify the *committed* state instead:

```bash
cd /home/dustin/projects/qmkonnect
git show 17e4f6f --stat                # Expected: Cargo.toml, src/platforms/mod.rs, docs/troubleshooting.md (+ this PRP + research).
                                         #   NO src/core/notifier.rs, src/main.rs, QMKonnect.iss, or docs/installation.md.
grep -n 'Win32_System_Com' Cargo.toml   # Expected: the feature line (Cargo.toml:98) + comment refs.
grep -n 'MessageBoxW' src/platforms/mod.rs   # Expected: only COMMENT references (the notify arm + show_toast doc explaining what was replaced);
                                         #   NO `MessageBoxW(` *call*. tray.rs has its OWN MessageBoxW (show_error_message) — OUT OF SCOPE.
grep -n 'show_toast\|build_toast_xml\|xml_escape' src/platforms/mod.rs   # Expected: defs + call sites (show_toast in notify arm; build_toast_xml+xml_escape in show_toast).
grep -n 'fn notify' src/platforms/mod.rs      # Expected: signature UNCHANGED: `pub fn notify(title: &str, body: &str)`
```

### Level 4: Feature-resolution spot check (Linux — runs, quick)
```bash
cd /home/dustin/projects/qmkonnect
cargo metadata --format-version 1 2>/dev/null \
  | grep -oE '"(Win32_System_Com|UI_Notifications|Data_Xml_Dom|Win32_UI_Shell)"' | sort -u
# Expected: all FOUR feature strings appear (S1's three + S2's Win32_System_Com). If Win32_System_Com
# is missing → the Cargo.toml edit didn't land. (cargo metadata always reflects resolved features
# and is the reliable cross-host check; cargo tree with the windows target may not resolve on Linux.)
```

### Level 5: Windows build + toast test + render (DEFERRED — AGENTS.md Windows dev loop)
```bash
# Run on a Windows host, from the CANONICAL path (Z:\projects\qmkonnect), NOT the C:\projects junction
# (AGENTS.md trap #2). Verify %CARGO_TARGET_DIR% is empty (trap #1) beforehand.
cd /z/projects/qmkonnect

# (a) Compile — resolves CoInitializeEx, XmlDocument::LoadXml, CreateToastNotification,
#     CreateToastNotifierWithId, ToastNotifier::Show. If "cannot find function `LoadXml`" or
#     `show` → you used snake_case (windows 0.52 is PascalCase). If "no function CreateToastNotifier
#     takes &HSTRING" → use CreateToastNotifierWithId (the WithId overload takes the AUMID).
cargo build --release

# (b) Run the cfg(windows) unit test (parses the escaped XML without showing a toast).
cargo test --bin qmkonnect -- --test-threads=1 toast_xml_is_well_formed_and_escapes_special_chars
# Expected: 1 passed. If it fails at XmlDocument::new() with CO_E_NOTINITIALIZED → the test's
#   CoInitializeEx line was removed/hoisted (it must be inside the #[test] fn, before new()).

# (c) Manual end-to-end render (needs P1.M4.T2.S1's Start Menu .lnk OR a temporary dev .lnk).
taskkill /IM qmkonnect.exe /F   # mandatory — single-instance mutex (AGENTS.md)
.\target\release\qmkonnect.exe -v   # run in your OWN session, NOT as a service
# In another terminal, break rules.toml (e.g. insert `= =` garbage), then switch window focus to
# trigger a re-parse. Expected: a toast ("QMKonnect: rules.toml invalid" + the parse error) slides in,
# auto-dismisses after ~7s, and is reviewable in Action Center. No modal, no focus steal.
# If NO toast appears but -v logs "show_toast: toast failed: …" → fix the error. If NO toast and NO
# log line → the AUMID has no Start Menu .lnk (expected until P1.M4.T2.S1); verify by creating a
# temporary dev .lnk with System.AppUserModel.ID = "Mulletware.QMKonnect" and retry.
```

## Final Validation Checklist

### Technical Validation
- [ ] Linux: `cargo build` succeeds (resolver validates `Win32_System_Com`).
- [ ] Linux: `cargo test --bin qmkonnect -- --test-threads=1` green, test count unchanged.
- [ ] `git show 17e4f6f --stat` = `Cargo.toml`, `src/platforms/mod.rs`, `docs/troubleshooting.md` (committed at HEAD; working-tree `git diff` empty except this PRP).
- [ ] Windows (DEFERRED, AGENTS.md loop): `cargo build --release` resolves the toast API + CoInitializeEx; the `toast_xml_…` cfg(windows) test passes; a real toast renders (with the .lnk from T2.S1).

### Feature Validation
- [ ] `notify()`'s Windows arm calls `show_toast(title, body)`; `MessageBoxW` is gone from `notify()` (`grep MessageBoxW src/platforms/mod.rs` returns nothing in notify; tray.rs's own MessageBoxW is out of scope and untouched).
- [ ] `show_toast` spawns a short-lived thread: `CoInitializeEx(STA)` → `build_toast_xml` → `XmlDocument::new` + `LoadXml` → `CreateToastNotification` → `CreateToastNotifierWithId(&APP_AUMID)` → `Show`, swallowing errors (`log::warn!`).
- [ ] `build_toast_xml` uses the `ToastText02` template; `xml_escape` escapes `& < > " '` (`&` first).
- [ ] The `#[cfg(all(test, target_os="windows"))]` test builds XML with special chars and asserts `LoadXml` succeeds (no `Show`).
- [ ] `notify()` signature + doc comment unchanged; Linux/macOS/not(any) arms byte-for-byte unchanged.
- [ ] `Cargo.toml`: `Win32_System_Com` added; S1's 3 features + all pre-existing features preserved; version `= "0.52.0"` unchanged; no new dep.
- [ ] `docs/troubleshooting.md`: the rules.toml-parse-error section notes Windows shows an auto-dismissing toast (Action Center).

### Code Quality Validation
- [ ] Follows repo conventions: snake_case free fns, cfg-gated private helpers (mirroring `osa_string`), local `use` inside the closure.
- [ ] Worker thread keeps the `HSTRING`/`XmlDocument`/`ToastNotification`/`ToastNotifier` alive across the calls (RAII; dropped before `CoUninitialize`).
- [ ] Comments explain WHY (COM-init vs. set_aumid; short-lived-thread-not-a-leak; XML-escape mandate; AUMID-vs-AppId; toast-needs-.lnk).
- [ ] No new dependencies; no version bumps; no `docs/installation.md` change (Mode A).

### Documentation & Deployment
- [ ] `docs/troubleshooting.md` toast note is user-facing, factual, one paragraph (no internal IDs).
- [ ] No new env vars / config keys / CLI flags.
- [ ] The dev-loop caveat (dev build without the .lnk → silent toast) is noted in the docs paragraph.

---

## Anti-Patterns to Avoid
- ❌ Don't call `CoInitializeEx` inline on the window-event thread (RPC_E_CHANGED_MODE risk); use the short-lived worker thread. (See Design Decision.)
- ❌ Don't use snake_case WinRT methods (`load_xml`, `show`) — they don't exist in windows 0.52; use `LoadXml`, `Show`.
- ❌ Don't use `ToastNotificationManager::CreateToastNotifier` (parameterless) — use `CreateToastNotifierWithId(&APP_AUMID)` so the toast is keyed to the registered AUMID.
- ❌ Don't construct `ToastNotification` with `::new()` or `Create` — use the `CreateToastNotification(&doc)` factory.
- ❌ Don't pass `PCWSTR`/`Vec<u16>` to the toast APIs — they take `&HSTRING` (`HSTRING::from(…)`); PCWSTR is only for S1's `set_aumid` (Win32).
- ❌ Don't skip `xml_escape` — the body is an arbitrary TOML error string and WILL break `LoadXml` unescaped.
- ❌ Don't change `notify()`'s signature, doc comment, or the Linux/macOS/no-op arms.
- ❌ Don't touch `notifier.rs`, `APP_AUMID`/`set_aumid`, `main.rs`, `QMKonnect.iss`, or `docs/installation.md`.
- ❌ Don't add `tauri-winrt-notification`/`notify-rust`/any new crate, or create a runtime `.lnk` (that's T2.S1).
- ❌ Don't expect a visible toast after S2 alone without the `.lnk` — `Show()` returns `Ok` and renders nothing until T2.S1; that's correct, not a bug.
- ❌ Don't run tests without `--test-threads=1` (AGENTS.md — shared global state).

---

## Confidence Score: 9/10

**Update since authoring:** the implementation is **committed at HEAD `17e4f6f`** and the
Linux gate (`cargo build`) is **verified green** (the resolver accepts `Win32_System_Com`,
the cfg-gated toast code is sound). This PRP is therefore an accurate record of code that
already compiles + matches the design; the residual risk is Windows-only (see below), not
authoring. Original (forward-looking) assessment retained for traceability:

The defect is precisely located (verbatim MessageBoxW arm quoted for replacement),
the windows-0.52 toast API sequence + exact signatures + the COM-init requirement
are verified in `research/winrs_toast_api.md` (with a full compilable recipe and
version-specific gotchas — PascalCase methods, `CreateToastNotifierWithId`,
`CreateToastNotification` factory, `&HSTRING`), and S1 has **already landed**
(verified: `APP_AUMID` at `mod.rs:134`, `set_aumid()` at `:149`, the 3 features in
Cargo.toml), so S2 consumes real, present outputs. The caller, dedup, docs anchor,
and cross-task file ownership (no overlap with P1.M3.T2.S1 / T2.S1) are all pinned.
The one deliberate deviation (keeping a short-lived thread for COM-init isolation
vs. the contract's literal "remove spawn") is engineering-justified and documented.
The 2-point reservation is the **validation asymmetry**: the implementing agent runs
on Linux and cannot compile the `#[cfg(target_os="windows")]` toast code or run the
`cfg(windows)` test, so the actual toast compile, the XML-parse test, and the
end-to-end render are validated only on Windows (AGENTS.md loop, deferred). The
Linux gates that DO run — the cargo resolver feature-name check (`Win32_System_Com`)
and the unchanged-arm regression — catch the two most likely authoring errors, and
the `research/winrs_toast_api.md` API table gives the exact fix if the Windows build
surfaces a wrong-method/wrong-overload error. Residual risk: the COM apartment
interaction with the app's existing event-loop threads is the one runtime unknown
the short-lived-thread pattern is specifically chosen to neutralize.