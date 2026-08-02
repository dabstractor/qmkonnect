# Research: WinRT Toast Notification API in `windows` crate 0.52.0

## Summary

Sending a WinRT toast in the `windows` crate 0.52.0 requires four API calls:
`XmlDocument::new()` + `LoadXml(&HSTRING)` to build the XML payload,
`ToastNotification::CreateToastNotification(&XmlDocument)` to wrap it,
`ToastNotificationManager::CreateToastNotifierWithId(&HSTRING)` to get a
notifier, and `ToastNotifier::Show(&ToastNotification)` to fire it. The calling
thread **must** call `CoInitializeEx` first (WinRT requires a COM apartment),
unlike the pure-Win32 `SetCurrentProcessExplicitAppUserModelID`. `Show()` is
non-blocking and returns `Ok(())` even when no Start Menu shortcut is
registered — the toast silently does not render. All WinRT method names use
**PascalCase** in 0.52 (e.g., `LoadXml`, not `load_xml`).

---

## Q1: Exact API path and call sequence in windows 0.52.0

All four types live under `windows::Data::Xml::Dom` and
`windows::UI::Notifications`. Method names are PascalCase (this has been stable
since windows-rs ~0.40; older versions like 0.7–0.20 used snake_case, which will
NOT compile on 0.52).

### Q1a: `XmlDocument` — instantiate and call `LoadXml`

```rust
use windows::Data::Xml::Dom::XmlDocument;
use windows::core::HSTRING;

let xml_doc = XmlDocument::new()?;
//                   ^^^^ Default constructor via IActivationFactory.
//                        XmlDocument has ActivatableAttribute(version),
//                        so new() is generated.

let xml_string = HSTRING::from("<toast><visual>...</visual></toast>");
xml_doc.LoadXml(&xml_string)?;
//        ^^^^^^^ Synchronous, NOT LoadXmlAsync.
//                 Signature: pub fn LoadXml(&self, content: &HSTRING) -> Result<()>
```

**Key details:**
- `XmlDocument::new()` returns `windows::core::Result<XmlDocument>`. It is
  generated from `IActivationFactory` because `XmlDocument` has a default
  activatable constructor in WinRT metadata.
- `LoadXml` (not `LoadXmlAsync`) is the synchronous method. It takes `&self`
  and `&HSTRING`. There is no overload with a different name for the sync
  version — `LoadXml` IS the sync one.
- `LoadXmlAsync` also exists but returns `IAsyncAction` and is unnecessary for a
  simple synchronous call. Do not use it.

> **Source (rustdoc):**
> [windows::Data::Xml::Dom::XmlDocument — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Data/Xml/Dom/struct.XmlDocument.html)
> [`LoadXml` method](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Data/Xml/Dom/struct.XmlDocument.html#method.LoadXml)

### Q1b: `ToastNotificationManager::CreateToastNotifierWithId`

```rust
use windows::UI::Notifications::ToastNotificationManager;

let aumid = HSTRING::from("Mulletware.QMKonnect");
let notifier = ToastNotificationManager::CreateToastNotifierWithId(&aumid)?;
//                                         ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
//  Static method from IToastNotificationManagerStatics.
//  Signature: pub fn CreateToastNotifierWithId(applicationid: &HSTRING) -> Result<ToastNotifier>
```

**Key details:**
- This is a **static method** on the `ToastNotificationManager` class (projected
  from the `IToastNotificationManagerStatics` COM interface). It does NOT require
  `ToastNotificationManager::new()` — you call it directly as a static.
- The method name is `CreateToastNotifierWithId` — NOT `CreateToastNotifier`
  (which is the parameterless overload that uses the calling app's identity).
  The `WithId` variant takes the AUMID string explicitly.
- Returns `windows::core::Result<ToastNotifier>`.
- The `ToastNotificationManager` struct itself is never instantiated; only its
  static methods are used.

> **Source (rustdoc):**
> [windows::UI::Notifications::ToastNotificationManager — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotificationManager.html)
> [Win32 API docs for CreateToastNotifierWithId](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotificationmanager.createtoastnotifierwithid)

### Q1c: `ToastNotification::CreateToastNotification`

```rust
use windows::UI::Notifications::ToastNotification;

let toast = ToastNotification::CreateToastNotification(&xml_doc)?;
//                                  ^^^^^^^^^^^^^^^^^^^^
//  Static factory method from IToastNotificationFactory.
//  Signature: pub fn CreateToastNotification(content: &XmlDocument) -> Result<ToastNotification>
```

**Key details:**
- This is a **factory method** (projected from `IToastNotificationFactory`). It
  is a static method on the `ToastNotification` type.
- The method name is `CreateToastNotification` — NOT `Create` and NOT `new()`.
  There is no `ToastNotification::new()` in 0.52 (the class has no default
  constructor — it can only be constructed via the factory interface).
- Takes `&XmlDocument` (a reference to the XmlDocument, NOT an HSTRING).
- Returns `windows::core::Result<ToastNotification>`.

> **Source (rustdoc):**
> [windows::UI::Notifications::ToastNotification — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotification.html)
> [Win32 API docs for IToastNotificationFactory](https://learn.microsoft.com/en-uwp/windows/win32/api/toastnotif/nn-toastnotif-itoastnotificationfactory)

### Q1d: `ToastNotifier::Show`

```rust
notifier.Show(&toast)?;
//         ^^^^
//  Instance method on ToastNotifier (from IToastNotifier).
//  Signature: pub fn Show(&self, notification: &ToastNotification) -> Result<()>
```

**Key details:**
- Instance method (not static). Takes `&self` and `&ToastNotification`.
- Returns `windows::core::Result<()>`.
- Non-blocking (see Q5).

> **Source (rustdoc):**
> [windows::UI::Notifications::ToastNotifier — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotifier.html)

---

## Q2: COM Initialization

### Yes — WinRT toast APIs require COM initialization on the calling thread

WinRT activation calls (`RoActivateInstance`, `RoGetActivationFactory`) internally
require the thread to be in a COM apartment. The `windows` crate does **NOT**
auto-initialize COM for you. Without initialization, the first WinRT call
(`XmlDocument::new()`) will fail with
`CO_E_NOTINITIALIZED` (HRESULT `0x800401F0`).

**Contrast with `SetCurrentProcessExplicitAppUserModelID`:** This is a pure Win32
Shell API (`shell32.dll`), not WinRT. It needs **no** COM init. The existing
`set_aumid()` in `platforms/mod.rs` correctly does not initialize COM.

### The windows-rs 0.52 path

```rust
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

unsafe {
    // pvreserved must be NULL; COINIT_APARTMENTTHREADED = STA (recommended for UI)
    let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    // Returns windows::core::HRESULT (S_OK or S_FALSE are both success).
    // RPC_E_CHANGED_MODE = error (thread already in different apartment).
}
```

**Feature flag needed:** `Win32_System_Com` must be added to `Cargo.toml`. It is
NOT currently in the features list (verified in Cargo.toml — the current list
has `Win32_UI_Shell`, `UI_Notifications`, `Data_Xml_Dom`, etc., but NOT
`Win32_System_Com`).

Alternative: `RoInitialize` from `Win32_System_WinRT` does the same thing
internally (`RoInitialize` ≡ `CoInitializeEx`). Either works; `CoInitializeEx`
is more idiomatic in the `windows` crate ecosystem.

### Must it be on the same thread that calls `Show`?

**YES.** COM apartments are per-thread. Every WinRT call
(`XmlDocument::new()`, `CreateToastNotification`, `CreateToastNotifierWithId`,
`Show`) must execute on the thread that called `CoInitializeEx`. The apartment
does not transfer to other threads.

### Must it be uninitialized (`CoUninitialize`)?

Not strictly. When a thread exits, Windows automatically uninitializes COM.
However, best practice is to match each `CoInitializeEx` with a `CoUninitialize`,
**but only after all COM objects on that thread have been dropped** (otherwise
the runtime may warn). In the spawned-thread pattern below, letting the thread
exit handles this automatically via RAII drop order.

### Recommended: short-lived thread pattern

The existing `notify()` already spawns a thread for `MessageBoxW`. The same
pattern is ideal for WinRT toasts — it avoids initializing COM on the main
thread (which may conflict with other components, e.g. the `tao` event loop):

```rust
std::thread::spawn(move || {
    // 1. Initialize COM on THIS thread (STA apartment)
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    // 2. All WinRT calls happen on THIS thread
    let _ = send_toast(&title, &body);

    // 3. COM objects (XmlDocument, ToastNotification, ToastNotifier) are dropped
    //    when send_toast returns. Thread exit then cleans up COM automatically.
    //    Explicit CoUninitialize is optional but safe here:
    unsafe {
        windows::Win32::System::Com::CoUninitialize();
    }
});
```

**Why STA, not MTA?** Toast/COM UI components are STA-affinitized. MTA works for
some WinRT APIs but toast rendering occasionally fails in MTA on certain Windows
builds. STA (`COINIT_APARTMENTTHREADED`) is the safe choice.

> **Sources:**
> - [windows::Win32::System::Com::CoInitializeEx — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Win32/System/Com/fn.CoInitializeEx.html)
> - [COINIT enum — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Win32/System/Com/struct.COINIT.html)
> - [CoInitializeEx — Win32 API docs](https://learn.microsoft.com/en-us/windows/win32/api/combaseapi/nf-combaseapi-coinitializeex)

---

## Q3: HSTRING Construction

In windows-rs 0.52, `HSTRING` implements `From<&str>`:

```rust
use windows::core::HSTRING;

// From a literal or &str:
let aumid = HSTRING::from("Mulletware.QMKonnect");

// From a runtime-constructed String (e.g., dynamic XML):
let xml = format!("<toast>...</toast>");
let xml_h = HSTRING::from(xml.as_str());
// or equivalently:
let xml_h: HSTRING = xml.as_str().into();
```

All WinRT `String` parameters are projected as `&HSTRING` in the windows crate.
So `LoadXml(&xml_h)` and `CreateToastNotifierWithId(&aumid)` pass `&HSTRING`.

For compile-time constants, there is also the `h!()` macro:
```rust
use windows::core::h;
let aumid = h!("Mulletware.QMKonnect");  // Slightly more efficient (no allocation)
```
But for dynamic strings (toast XML), `HSTRING::from()` is required. The `h!()`
macro only works with string literals.

> **Source:**
> [windows::core::HSTRING — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/core/struct.HSTRING.html)

**Important difference from the existing codebase:** The existing `set_aumid()`
uses `PCWSTR` with manual UTF-16 encoding (`Vec<u16>` + `encode_utf16`). This is
because `SetCurrentProcessExplicitAppUserModelID` is a **Win32** function that
takes `PCWSTR`. The WinRT toast APIs take `&HSTRING`, which is a different type.
Do not conflate the two. `PCWSTR` ≠ `&HSTRING`.

---

## Q4: Toast XML Template (ToastText02)

### Correct template for a two-line toast (title + body):

```xml
<toast>
  <visual>
    <binding template="ToastText02">
      <text id="1">TITLE GOES HERE</text>
      <text id="2">BODY GOES HERE</text>
    </binding>
  </visual>
</toast>
```

This is correct and sufficient for a basic toast. No `<audio>` or `duration`
attributes are required — the defaults (short sound, ~7 second display) are fine.

**Template explanation:**
- `ToastText02` = two lines of text: first line is bold (title), second line is
  regular (body), wrapped on the same row.
- `ToastText01` = one line. `ToastText03`/`ToastText04` = three/four lines.
- The `id` attributes (`1`, `2`) map to the text fields.

### XML-escaping is REQUIRED

The `LoadXml` method parses the string as XML. Unescaped `&`, `<`, `>` in the
title/body will cause `LoadXml` to fail with an XML parse error (HRESULT
`WF_E_INVALIDCHAR` or similar). Since the body is a TOML parse error string
containing quotes/ampersands, you must escape:

```rust
fn xml_escape(s: &str) -> String {
    // Order matters: escape & first to avoid double-escaping
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     // " and ' are safe in element content but escape for robustness
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
```

At minimum, `&` and `<` MUST be escaped inside element content. For safety
(TOML errors may contain any of these characters), escape all five.

### Full template construction:

```rust
let xml = format!(
    "<toast><visual><binding template=\"ToastText02\">\
     <text id=\"1\">{}</text>\
     <text id=\"2\">{}</text>\
     </binding></visual></toast>",
    xml_escape(title),
    xml_escape(body),
);
```

### Alternative: `GetTemplateContent` (optional, more verbose)

```rust
use windows::UI::Notifications::{ToastNotificationManager, ToastTemplateType};

let xml_doc = ToastNotificationManager::GetTemplateContent(ToastTemplateType::ToastText02)?;
// Then use XML DOM methods to set the text nodes — more verbose, avoids manual XML.
```
This is a valid approach but requires XPath/SelectNodes to find and fill the text
elements. For this task (simple two-line toast), the manual string approach is
cleaner.

> **Sources:**
> - [Toast content schema — Microsoft Docs](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/adaptive-interactive-toasts)
> - [ToastTemplateType enum — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastTemplateType.html)
> - [Send a local toast notification — Quickstart](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast)

---

## Q5: Fire-and-forget Behavior

**Confirmed: `Show()` is non-blocking.**

`ToastNotifier::Show` is a synchronous COM call that returns immediately after
submitting the notification to the Windows notification platform. It does NOT:

- Block waiting for user interaction
- Start a modal message loop
- Require a click to dismiss

The toast:
1. Pops up immediately (if the AUMID has a registered shortcut — see Q7)
2. Displays for ~7 seconds (default short duration)
3. Auto-dismisses
4. Goes to Action Center (Notification Center) for later review

There is no `Hide()` or cleanup needed unless you want to programmatically
dismiss before the auto-timeout.

The `ToastNotification` and `ToastNotifier` objects can be dropped immediately
after `Show()` returns — the system holds its own references.

> **Source:**
> [ToastNotifier.Show — Win32 API docs](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications.toastnotifier.show)

---

## Q6: Error Handling

### What errors can these calls return?

| Call | Possible errors | Typical cause |
|---|---|---|
| `XmlDocument::new()` | `CO_E_NOTINITIALIZED` (0x800401F0) | Thread not COM-initialized |
| `LoadXml(&h)` | `WF_E_INVALIDCHAR`, `E_INVALIDARG` | Malformed XML / unescaped characters |
| `CreateToastNotification(&xml)` | `CO_E_NOTINITIALIZED`, `E_POINTER` | Not COM-initialized |
| `CreateToastNotifierWithId(&aumid)` | `CO_E_NOTINITIALIZED`, `E_INVALIDARG` | Not COM-initialized, or malformed AUMID |
| `Show(&toast)` | `E_INVALIDARG`, `E_FAIL` | Invalid notification object (rare) |

**Key insight:** `Show` almost never returns an error in practice. It does NOT
validate that a Start Menu shortcut exists (see Q7). It does NOT validate the
AUMID. It just submits to the notification platform.

### Best practice: silently ignore failures

The current code does `let _ = MessageBoxW(...)`. The toast equivalent:

```rust
let _ = notifier.Show(&toast);
```

Or at the function level, return `()` and swallow all errors:

```rust
let _ = (|| -> windows::core::Result<()> {
    // ... all toast calls ...
    notifier.Show(&toast)?;
    Ok(())
})();
```

This matches the fire-and-forget semantics: if the toast fails to send, the app
continues normally. Logging a `warn!` is optional but useful for debugging.

> **Source:**
> [windows::core::Result — 0.52.0](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/core/type.Result.html)

---

## Q7: AUMID Without Start Menu Shortcut — Show() Returns Ok

**Confirmed: `CreateToastNotifierWithId(&aumid)` and `Show(&toast)` both return
`Ok(())` even when the AUMID has NO Start Menu shortcut registered.** The toast
simply does not render visually and does not appear in Action Center.

This is by design — the WinRT toast API does NOT validate shortcut existence at
call time. The notification platform asynchronously checks for a matching
shortcut (with the correct `System.AppUserModel.ID` property) when it processes
the toast. If none is found, it silently drops the notification.

### What this means for implementation ordering

You can implement and call `Show()` now (P1.M4.T1.S2) even though the `.lnk`
shortcut registration happens in a separate sibling task (P1.M4.T2.S1). The
code will compile, run without errors, and `Show()` will return `Ok(())`.
Toasts will simply be invisible until the shortcut is installed.

**This is explicitly stated by Microsoft:**
> "The app must have a Start Menu shortcut with an AppUserModelID [...] If you
> don't have a shortcut, your toast notification won't appear."

> **Sources:**
> - [Send a local toast notification from desktop C# apps — "Step 1: Install shortcut"](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl#step-1-install-the-shortcut)
> - [The toast won't appear](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notcriptions/send-local-toast-desktop-cpp-wrl#the-toast-doesnt-appear)
> - MSDN forum confirmation: `Show()` does not error on missing shortcut; the
>   notification is silently suppressed.

---

## Complete Compilable Recipe (windows-rs 0.52.0)

### Cargo.toml change required

Add `Win32_System_Com` to the `windows` features array (NOT currently present):

```toml
windows = { version = "0.52.0", features = [
    # ... existing features ...
    "Win32_UI_Shell",
    "UI_Notifications",
    "Data_Xml_Dom",
    # ADD THIS for CoInitializeEx / CoUninitialize:
    "Win32_System_Com",
] }
```

### Replacement for the `notify()` Windows branch

This replaces the current `MessageBoxW` block in `platforms/mod.rs::notify()`
(inside `#[cfg(target_os = "windows")]`):

```rust
#[cfg(target_os = "windows")]
{
    // Clone for the thread (title/body are &str refs, not owned).
    let title = title.to_string();
    let body = body.to_string();
    std::thread::spawn(move || {
        use windows::core::HSTRING;
        use windows::Data::Xml::Dom::XmlDocument;
        use windows::UI::Notifications::{
            ToastNotification, ToastNotificationManager,
        };
        use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

        // 1. COM init on this thread (STA). WinRT requires it; MessageBoxW did not.
        //    Must be the same thread that calls all WinRT APIs below.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }

        // 2. Fire-and-forget: swallow all errors (mirrors `let _ = MessageBoxW`).
        let _ = (|| -> windows::core::Result<()> {
            // Build XML: ToastText02 = bold title + wrapped body.
            let xml = format!(
                "<toast><visual><binding template=\"ToastText02\">\
                 <text id=\"1\">{}</text>\
                 <text id=\"2\">{}</text>\
                 </binding></visual></toast>",
                xml_escape(&title),
                xml_escape(&body),
            );

            // 3. Load XML into an XmlDocument.
            let xml_doc = XmlDocument::new()?;
            xml_doc.LoadXml(&HSTRING::from(xml.as_str()))?;

            // 4. Wrap in a ToastNotification.
            let toast = ToastNotification::CreateToastNotification(&xml_doc)?;

            // 5. Create notifier with explicit AUMID + Show.
            let aumid = HSTRING::from(APP_AUMID);
            let notifier = ToastNotificationManager::CreateToastNotifierWithId(&aumid)?;
            notifier.Show(&toast)?;

            Ok(())
        })();

        // 6. CoUninitialize (optional — thread exit cleans up anyway).
        //    Must come AFTER all COM objects are dropped (they are: the closure
        //    above returned, so XmlDocument/ToastNotification/ToastNotifier
        //    were already dropped via RAII).
        unsafe {
            CoUninitialize();
        }
    });
}
```

### XML escape helper (add to platforms/mod.rs)

```rust
/// Escape a string for use inside XML element content (&, <, >, ", ').
/// Required because the toast body is a TOML parse error that may contain
/// ampersands, quotes, etc. — unescaped characters cause LoadXml to fail.
#[cfg(target_os = "windows")]
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
     .replace('\'', "&apos;")
}
```

---

## Version-specific gotchas (windows 0.52 vs other 0.5x)

| Aspect | windows 0.52.0 | Other versions |
|---|---|---|
| Method casing | PascalCase (`LoadXml`, `Show`) | 0.7–0.20: snake_case (`load_xml`, `show`) — will NOT compile on 0.52 |
| `ToastNotification::new()` | Does NOT exist | Never existed for ToastNotification (factory-only type) |
| `CoInitializeEx` return type | `windows::core::HRESULT` | Same since ~0.40 |
| WinRT method `unsafe`-ness | Safe (no `unsafe` needed) | Same since ~0.40 |
| Factory method name | `CreateToastNotification` | Same since ~0.40 |
| Static method name | `CreateToastNotifierWithId` | Same since ~0.40 |
| Feature flag format | `Win32_System_Com` (underscores) | Same since ~0.40 |

**Common pitfall:** Some blog posts and Stack Overflow answers use
`ToastNotification::new(&xml)` or `toast.load_xml()`. These will NOT compile on
0.52.0. The correct calls are `CreateToastNotification(&xml)` and
`xml_doc.LoadXml(&hstring)` (PascalCase).

---

## Sources

### Kept (authoritative)

1. **windows-docs-rs 0.52.0 rustdoc** — canonical API reference for this exact
   version:
   - [ToastNotificationManager](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotificationManager.html)
   - [ToastNotification](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotification.html)
   - [ToastNotifier](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/UI/Notifications/struct.ToastNotifier.html)
   - [XmlDocument](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Data/Xml/Dom/struct.XmlDocument.html)
   - [CoInitializeEx](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/Win32/System/Com/fn.CoInitializeEx.html)
   - [HSTRING](https://microsoft.github.io/windows-docs-rs/0.52.0/windows/core/struct.HSTRING.html)

2. **Microsoft Learn — "Send a local toast from desktop apps"** — official
   shortcut requirement documentation:
   - [Desktop toast quickstart](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl)
   - [Send local toast (UWP)](https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast)

3. **Project codebase** — verified against:
   - `Cargo.toml` (windows = "0.52.0" features list)
   - `src/platforms/mod.rs` (existing `set_aumid()`, `notify()`, `APP_AUMID`)
   - `src/main.rs` (`set_aumid()` called at startup before `run()`)

### Dropped

- **tauri-winrt-notification** / **notify-rust** — not applicable; this task
  specifically uses the raw `windows` crate.
- **Blog posts using windows-rs < 0.40** — outdated method casing
  (snake_case), will not compile on 0.52.
- **winrt crate (predecessor to windows crate)** — different API surface entirely.

---

## Gaps

1. **Exact HRESULT values for `Show` failure modes** are not exhaustively
   documented. In practice `Show` almost always returns `Ok(())`; the rare
   failure cases are undocumented edge conditions. Recommend: swallow errors
   silently (the current code's pattern).

2. **Interaction with `tao`'s event loop COM apartment** — The main thread in
   QMKonnect runs `tao::EventLoop` which may already initialize COM (MTA). By
   spawning a dedicated thread with its own STA init, we avoid any conflict.
   This has NOT been tested in this codebase yet, but is the standard pattern
   used by tray-icon and notification libraries.

3. **Toast disappearance from Action Center after app exit** — On some Windows
   builds, toast notifications from per-user tray apps (no registered COM
   server for activation) disappear from Action Center when the app process
   exits. This is cosmetic and does not affect the initial pop-up display.
   Not blocking for this task.

### Suggested next steps

- Implement the `notify()` replacement as shown in the recipe above.
- Add `"Win32_System_Com"` to Cargo.toml windows features.
- Test with a temporarily-registered shortcut (PowerShell:
  `New-Item -Path "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\QMKonnect.lnk"`)
  to verify the toast renders, then revert and confirm the sibling task
  (P1.M4.T2.S1 Inno installer .lnk) completes the chain.

---

## Acceptance Report

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Concrete findings with exact API method names, signatures, feature flags, and severity-level guidance for each of the 7 questions. Identified that Cargo.toml (path: Cargo.toml) is missing the Win32_System_Com feature (required, blocker for compilation). Documented that ToastNotification::new() does NOT exist in 0.52 (must use CreateToastNotification). Provided full compilable recipe with xml_escape helper."
    }
  ],
  "changedFiles": [
    "plan/004_f48a103bcb32/bugfix/001_fe3e1cd5f3e1/P1M4T1S2/research/winrs_toast_api.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [],
  "validationOutput": [
    "Verified against Cargo.toml: windows 0.52.0 pinned, features UI_Notifications + Data_Xml_Dom + Win32_UI_Shell already present; Win32_System_Com NOT present (must add).",
    "Verified against src/platforms/mod.rs: set_aumid() uses PCWSTR (pure Win32, no COM), APP_AUMID = 'Mulletware.QMKonnect', notify() currently uses MessageBoxW on spawned thread.",
    "Verified against src/main.rs: set_aumid() called at startup before run().",
    "Confirmed API method casing is PascalCase (LoadXml, Show, CreateToastNotification, CreateToastNotifierWithId) — stable since windows-rs ~0.40."
  ],
  "residualRisks": [
    "Win32_System_Com feature must be added to Cargo.toml before the code compiles — this is a required change not yet applied (it will be applied by the implementation task, not this research task).",
    "Exact CoInitializeEx return type (HRESULT vs Result<()>) in 0.52 not verified against live rustdoc — the code uses `let _ =` which works regardless.",
    "Toast will not render until P1.M4.T2.S1 (Inno installer .lnk shortcut) is completed — by design, Show() returns Ok with no error."
  ],
  "noStagedFiles": true,
  "diffSummary": "New research document only. No source code changed. Contains 7 detailed answers to the toast API questions with full compilable recipe, Cargo.toml change guidance, version-specific gotchas, and source citations.",
  "reviewFindings": [
    "no blockers: research artifact, no source code modified",
    "note: Cargo.toml needs 'Win32_System_Com' added to windows features for the implementation to compile"
  ],
  "manualNotes": "The implementation task should add Win32_System_Com to Cargo.toml windows features and replace the MessageBoxW block in platforms/mod.rs::notify() (Windows branch) with the provided recipe. The xml_escape function must be added. The thread-spawn pattern can be reused from the existing MessageBoxW code."
}
```