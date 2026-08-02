# Windows Desktop Notification (`platforms::notify`) — Research

Finding #3 (bug-hunt report) claims: *"platforms::notify on Windows uses a modal
`MessageBoxW(HWND(0), …, MB_OK)` on a spawned thread, not a toast. It's
desktop-modal (steals focus, needs a click) and leaks a thread until dismissed."*

## Verdict: CONFIRMED (severity: low–medium)

Every element of the claim is accurate. It is also a **spec violation**: the UI
spec explicitly mandates a **toast** on Windows.

---

## 1. Where the code lives

The Windows `notify` body is **not in `src/platforms/windows.rs`** — it lives in
the platform-dispatch `notify()` in **`src/platforms/mod.rs`** (lines 126–172).
`windows.rs` only uses `HWND` for the window monitor, never for notifications.

```rust
// src/platforms/mod.rs:126-172
/// Best-effort, non-blocking desktop notification (fire-and-forget). ...
pub fn notify(title: &str, body: &str) {
    ...
    #[cfg(target_os = "windows")]
    {
        // Dep-free stand-in for a WinRT toast (a true toast needs an
        // AppUserModelID + Start Menu shortcut to render). Non-blocking via a
        // spawned thread; mirrors tray.rs's MessageBoxW idiom.
        let body_w: Vec<u16>  = body.encode_utf16().chain(std::iter::once(0)).collect();
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

### Why the three sub-claims are all true

| Sub-claim | Evidence | True? |
|---|---|---|
| Modal `MessageBoxW(HWND(0), …, MB_OK)` not a toast | `mod.rs:160` — `MB_OK \| MB_ICONERROR` with `HWND(0)` owner | ✅ |
| Desktop-modal / steals focus / needs a click | A null-owner `MessageBoxW` is created as a top-level window that the message-box manager foreground-activates; it runs its own `GetMessage`/modal loop and only returns on `IDOK`/close. With no `MB_TASKMODAL`/owner it blocks the desktop's foreground. | ✅ |
| On a spawned thread | `mod.rs:154` — `std::thread::spawn(move \| \| { … })` | ✅ |
| Thread leaks until dismissed | The spawned `JoinHandle` is **dropped** (detached), so the OS thread is alive until `MessageBoxW` returns — i.e. until the user clicks **OK**. No cancel/timeout path. | ✅ |
| "mirrors tray.rs's MessageBoxW idiom" (comment) | Partially misleading. `tray.rs:1143-1156` (`show_error_message`) runs `MessageBoxW` **synchronously on the caller's thread** (truly blocking the caller). `platforms::notify` instead *detaches* it to a leaky thread. Same primitive, different (worse) lifecycle. | nuance |

Severity is tempered by: the notification fires **at most once per broken
state** (see §3), and never blocks the app's main thread. The user-impact window
is a single unexpected modal that grabs focus and won't go away on its own.

## 2. What triggers a notification (the only caller)

There is **exactly one** call site of `crate::platforms::notify`:

```rust
// src/core/notifier.rs:1013-1041  (host_context_for_window)
fn host_context_for_window(window_info: &WindowInfo, verbose: bool) -> Option<HostContext> {
    if !host_capable() { return None; }          // legacy/offline -> string-only
    let path = get_rules_paths().into_iter().find(|p| p.exists())?;
    match parse_rules(&path) {
        Ok(r) => { RULES_INVALID_NOTIFIED.store(false, Ordering::SeqCst); r }
        Err(e) => {
            if !RULES_INVALID_NOTIFIED.swap(true, Ordering::SeqCst) {
                crate::platforms::notify("QMKonnect: rules.toml invalid", &format!("{e}"));
            }
            ...
            return None;                          // graceful string-only fallback
        }
    }
    ...
}
```

So the **only** thing that ever calls `platforms::notify` is a malformed
`rules.toml` parse, surfaced as **"QMKonnect: rules.toml invalid"** carrying the
parse error. There is no config-error path, no broken-toml-from-installer path,
no per-event toast — just this single host-rules failure.

Note: the **Linux tray** (`linux_tray.rs:845 fn notify`) has its *own* local
`notify` that shells out to `notify-send`; it does **not** call
`platforms::notify`. The **Windows tray** (`tray.rs`) **never** calls
`platforms::notify` at all (`grep` for `platforms::notify` in `tray.rs`: no
matches). So on Windows, `platforms::notify` is reached *only* through
`host_context_for_window`.

## 3. Dedup logic (works correctly)

```rust
// src/core/notifier.rs:264-267
static RULES_INVALID_NOTIFIED: AtomicBool = AtomicBool::new(false);
```

- On the first parse failure: `!RULES_INVALID_NOTIFIED.swap(true, …)` is `true`
  → fires the notification **once**, then sets the flag.
- Subsequent failures (every window focus change re-parses `rules.toml`): the
  flag is already `true` → `swap` returns `true` → `!true == false` → **no
  repeat**.
- On the first *successful* parse after a break: `RULES_INVALID_NOTIFIED.store(false)`
  **re-arms** the flag (line 1027), so a *later* breakage notifies again.
- Result: **at most one notification per broken state** — exactly as the doc
  comment promises (HOST_RULES.md §7). The dedup is sound; the bug is purely the
  *delivery mechanism*, not the rate-limiting.

## 4. Spec requirements (this is the real issue)

### `spec/UI.md` §2.3 (line ~139) — explicit, violated
> "… also fires an automatic **"rules.toml invalid"** notification when
> `rules.toml` fails to parse (host rules fall back to string-only — never
> silent). macOS uses `NSUserNotification`/`UNUserNotificationCenter`;
> **Windows a toast — same trigger.**"

The spec mandates a **toast**. The implementation ships a `MessageBoxW` modal.
**This is a clear, low-ambiguity spec deviation.**

### `spec/HOST_RULES.md` §7 (line ~426-427)
> "… fire a **desktop notification** (`notify-send` on Linux,
> `NSUserNotification` on macOS, **toast on Windows**) carrying the parse error …"

Same requirement, restated. Windows → toast.

### `spec/PLATFORMS.md` §7
Section 7 of `PLATFORMS.md` is "Where Each Monitor Runs (thread summary)" — it
does **not** cover notifications (the report's reference to "section 7
(notifications)" is slightly off; the notification spec lives in `UI.md` §2.3
and `HOST_RULES.md` §7). No conflict — both pin Windows to a toast.

### `src/platforms/mod.rs:150` (the author's own admission)
> "Dep-free stand-in for a WinRT toast (a true toast needs an AppUserModelID +
> Start Menu shortcut to render)."

The author shipped this as a *known* stop-gap; the comment already identifies the
two prerequisites a real fix needs: an **AUMID** and a **Start Menu shortcut**.

## 5. What toast/notification APIs are available in the dependency tree

**Short answer: none today.** A real toast requires adding a feature and some
plumbing.

### `Cargo.toml` → `[target.'cfg(target_os="windows")'.dependencies]` (lines 65-79)
The `windows = "0.52.0"` crate is pulled in with these features only:

```
Win32_Foundation, Win32_System_Threading, Win32_System_DataExchange,
Win32_System_Memory, Win32_UI_WindowsAndMessaging, Win32_UI_Accessibility,
Win32_System_LibraryLoader, Win32_Security, Win32_UI_Controls,
Win32_Graphics_Gdi, Win32_System_Console, Win32_System_Registry
```

**Missing:** `Win32_UI_Notification` (the `ToastNotificationManager`,
`ToastNotification`, `ToastNotifier`, `ToastNotificationManagerCompat`,
`XmlDom` for the toast XML payload). Also missing: the WinRT interop helpers
(`Data_Xml_Dom`), which a classic toast needs to build the XML payload.

**No standalone toast crate** is in the tree either — no `winrt-notification`,
`toast`, `tauri-winrt-notification`, or `notify-rust`. (`notify-rust` is
explicitly *rejected* for Linux in `spec/LINUX.md` §7.3 because of a nested-
tokio-runtime footgun; that rationale is Linux-specific and does not preclude a
Windows-native toast.)

### Fix prerequisites (for whoever implements it)
1. **Cargo.toml**: add `Win32_UI_Notification` (and likely
   `Win32_System_Com`, `Data_Xml_Dom`) to the `windows` features list.
2. **AUMID + shortcut**: a WinRT toast will not render unless the process has a
   registered **AppUserModelID** and a **Start Menu shortcut** advertising that
   AUMID. The Inno installer (`packaging/windows/inno/`) is the natural place to
   create the `.lnk` (it already installs the tray app). The running process must
   also set its AUMID via `SetCurrentProcessExplicitAppUserModelID`
   (`Win32_UI_Shell` / `propsys`).
3. **Replace** the `std::thread::spawn(MessageBoxW)` block in `mod.rs:150-166`
   with a `ToastNotificationManager` + XML payload build. Toasts are
   fire-and-forget by design (no thread leak, no modal), so the `thread::spawn`
   wrapper can go away or stay as a thin off-main-thread guard.
4. The dedup in `notifier.rs` and the "rules.toml invalid" trigger are both
   correct and **should not change**.

### Quick mitigation if a real toast is deferred
- Drop `MB_ICONERROR` severity and/or add `MB_TASKMODAL | MB_SETFOREGROUND |
  MB_TOPMOST` only if a non-blocking option is needed — **but no `MessageBoxW`
  flag makes it a toast**; the modal/focus-grab behavior is inherent. The only
  non-modal options are (a) a true toast, or (b) logging to Event Log only
  (already happens for other paths) and dropping the popup entirely. Neither (b)
  matches the spec ("never silent"). So the toast is the right fix.

## 6. Cross-platform consistency (why Windows is the odd one out)

| Platform | `platforms::notify` impl | Modal? | Thread? | Matches spec? |
|---|---|---|---|---|
| Linux | shell `notify-send --app-name=QMKonnect --icon=input-keyboard` (`mod.rs:138-143`) | No (Freedesktop notification) | blocking spawn-and-wait of a short-lived process | ✅ |
| macOS | shell `osascript -e 'display notification …'` (`mod.rs:144-149`) | No (Notification Center) | blocking spawn-and-wait | ✅ (spec says `NSUserNotification`; osascript hits NC — acceptable) |
| **Windows** | **`thread::spawn(MessageBoxW(HWND(0), …, MB_OK\|MB_ICONERROR))`** (`mod.rs:150-166`) | **Yes (desktop-modal)** | **detached, leaks until click** | **❌ spec says toast** |

So the bug is Windows-specific and isolated to ~17 lines in `mod.rs`. Linux and
macOS already do the right thing; the Windows branch is the lone deviation.

---

## Summary for the fixer

- **Single file to change:** `src/platforms/mod.rs` lines **150–166** (the
  `#[cfg(target_os = "windows")]` arm of `notify()`).
- **Single trigger:** `host_context_for_window` in `src/core/notifier.rs:1035`
  ("rules.toml invalid"). Dedup is correct — leave it alone.
- **Spec basis:** `spec/UI.md` §2.3 (line 139) + `spec/HOST_RULES.md` §7 →
  Windows must use a toast.
- **Missing dep:** add `Win32_UI_Notification` (+ `Data_Xml_Dom`/`Win32_System_Com`)
  to `Cargo.toml`; also need AUMID + Start-Menu shortcut (Inno installer) for
  the toast to actually render.
- **Do not touch:** `RULES_INVALID_NOTIFIED`, the Linux/macOS arms, the
  `notify()` signature, the "rules.toml invalid" string.