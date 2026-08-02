# AUMID Property-Store Recipe (C# / PowerShell Add-Type)

**Purpose.** Set `System.AppUserModel.ID` (AUMID) on an EXISTING Start Menu `.lnk`
so WinRT toasts brand as the app and actually render. The Inno `[Icons]` section
already CREATES the `.lnk`; this helper only OPENS it, sets the property, saves.

**Why a helper, not Inno's own WScript.Shell.** Setting `System.AppUserModel.ID`
requires `IPropertyStore::SetValue` — a **vtable** COM interface. Inno's Pascal
Script `CreateOleObject('WScript.Shell')` only exposes **IDispatch (automation)**
objects; `IPropertyStore` is not IDispatch. So the property cannot be set from
Inno's scripting layer directly. A small PowerShell + `Add-Type` (C# P/Invoke)
helper is the documented, robust path. (Verified against Microsoft's
"Send a local toast notification from desktop" quickstart pattern.)

---

## The exact API sequence

1. `CoCreateInstance(CLSID_ShellLink, …, CLSCTX_INPROC_SERVER, IID_IPersistFile)` → `IPersistFile`
2. `IPersistFile.Load(path, STGM_READWRITE)` — open the existing `.lnk`
3. `QI` the same object for `IID_IPropertyStore`
4. `IPropertyStore.SetValue(PKEY_AppUserModel_ID, propvariant(VT_LPWSTR, aumid))`
5. `IPropertyStore.Commit()`
6. `IPersistFile.IsDirty()` / `IPersistFile.Save(path, fRemember:true)` — **REQUIRED** (Commit alone does NOT persist to the `.lnk` file)

## Exact GUIDs / constants (verbatim)

| Symbol | Value |
|---|---|
| `CLSID_ShellLink` | `{00021401-0000-0000-C000-000000000046}` |
| `IID_IPersistFile` | `{0000010B-0000-0000-C000-000000000046}` |
| `IID_IPropertyStore` | `{886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99}` |
| `PKEY_AppUserModel_ID` fmtid | `{9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3}`, pid **5** |
| `VT_LPWSTR` | `31` (0x1F) |
| `STGM_READWRITE` | `0x00000002` |
| `CLSCTX_INPROC_SERVER` | `0x1` |
| `S_OK` | `0` |

---

## Verbatim C# (passed to PowerShell `Add-Type -TypeDefinition`)

```csharp
using System;
using System.Runtime.InteropServices;

namespace QMKonnect {
  public static class ShortcutAumid {
    // ---- COM interface declarations -------------------------------------
    [ComImport, Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IPropertyStore {
      uint GetCount([Out] out uint cProps);
      uint GetAt([In] uint iProp, out PropertyKey pkey);
      uint GetValue([In] ref PropertyKey key, [Out] PropVariant pv);
      uint SetValue([In] ref PropertyKey key, [In] ref PropVariant pv);
      uint Commit();
    }

    [ComImport, Guid("0000010B-0000-0000-C000-000000000046"),
     InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
    public interface IPersistFile {
      uint GetClassID([Out] out Guid pClassID);
      uint IsDirty();
      uint Load([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName,
                [In] uint dwMode);
      uint Save([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName,
                [In, bool fRemember);
      uint SaveCompleted([In, MarshalAs(UnmanagedType.LPWStr)] string pszFileName);
      uint GetCurFile([Out, MarshalAs(UnmanagedType.LPWStr)] out string ppszFileName);
    }

    // PROPERTYKEY: fmtid (GUID) + pid (uint)
    [StructLayout(LayoutKind.Sequential, Pack = 4)]
    public struct PropertyKey {
      public Guid fmtid; public uint pid;
      public PropertyKey(Guid fmtid, uint pid) { this.fmtid = fmtid; this.pid = pid; }
    }

    // PROPVARIANT — LayoutKind.Explicit is MANDATORY (#1 cause of failure).
    // VARTYPE at byte 0, three reserved WORDs (bytes 2..7), union pointer at byte 8.
    // 24 bytes on x64 (pad at byte 16). VT_LPWSTR = 31.
    [StructLayout(LayoutKind.Explicit)]
    public struct PropVariant {
      [FieldOffset(0)]  public ushort vt;
      [FieldOffset(2)]  public ushort wReserved1;
      [FieldOffset(4)]  public ushort wReserved2;
      [FieldOffset(6)]  public ushort wReserved3;
      [FieldOffset(8)]  public IntPtr pwszVal;   // VT_LPWSTR pointer
      [FieldOffset(16)] public IntPtr pad;        // explicit size to 24 bytes

      public static PropVariant FromString(string s) {
        var pv = new PropVariant { vt = 31 };           // VT_LPWSTR
        pv.pwszVal = Marshal.StringToCoTaskMemUni(s);   // caller must free
        return pv;
      }
      public void Clear() {
        if (vt == 31 && pwszVal != IntPtr.Zero)
          Marshal.FreeCoTaskMem(pwszVal);
        vt = 0; pwszVal = IntPtr.Zero;
      }
    }

    [DllImport("ole32.dll")]
    static extern uint CoCreateInstance(
        [In] ref Guid rclsid, [In] IntPtr pUnkOuter, [In] uint dwClsContext,
        [In] ref Guid riid, [Out, MarshalAs(UnmanagedType.Interface)] out object ppv);

    static readonly Guid CLSID_ShellLink =
        new Guid("00021401-0000-0000-C000-000000000046");
    static readonly Guid IID_IPersistFile =
        new Guid("0000010B-0000-0000-C000-000000000046");
    static readonly Guid IID_IPropertyStore =
        new Guid("886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99");
    static readonly PropertyKey PKEY_AppUserModel_ID =
        new PropertyKey(new Guid("9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"), 5);

    // ---- Public entrypoint ---------------------------------------------
    // Returns 0 on success, non-zero HRESULT / 1 on failure.
    public static int Set(string lnkPath, string aumid) {
      object obj; Guid iid = IID_IPersistFile;
      uint hr = CoCreateInstance(ref CLSID_ShellLinkGuid_local, IntPtr.Zero, 0x1,
                                 ref iid, out obj);
      if (hr != 0) return (int)hr;
      try {
        var pf = (IPersistFile)obj;
        if (pf.Load(lnkPath, 0x00000002) != 0) return unchecked((int)0x80004005); // E_FAIL
        // The ShellLink CoClass implements both IPersistFile and IPropertyStore.
        // Marshal the same object to the IPropertyStore interface.
        var ps = (IPropertyStore)obj;
        var pv = PropVariant.FromString(aumid);
        try {
          if (ps.SetValue(ref PKEY_AppUserModel_ID, ref pv) != 0)
            return unchecked((int)0x80004005);
          ps.Commit();
        } finally { pv.Clear(); }
        // Commit flushes the in-memory store ONLY — persist the .lnk to disk:
        if (pf.IsDirty() == 0) {   // S_OK == dirty
          if (pf.Save(lnkPath, true) != 0) return unchecked((int)0x80004005);
        }
        return 0;
      } finally { Marshal.ReleaseComObject(obj); }
    }
  }
}
```

> **NOTE on the snippet above:** a few compile-affecting tokens were abbreviated
> for readability (e.g. the `Save` signature line break, the
> `CLSID_ShellLinkGuid_local` placeholder). The **implementing agent MUST use the
> fully-correct, compileable version in `packaging/windows/inno/set_aumid.ps1`**
> (Task 1 of the PRP gives the production script verbatim). The struct layout,
> GUIDs, call order, and the Commit-then-Save requirement here are all
> authoritative.

## PowerShell wrapper shape (`set_aumid.ps1`)

```powershell
param(
    [Parameter(Mandatory=$true, Position=0)][string]$LnkPath,
    [Parameter(Mandatory=$true, Position=1)][string]$Aumid
)
$ErrorActionPreference = 'Stop'
# Guard: Add-Type throws if the type already exists in the session (re-runs).
if (-not ('QMKonnect.ShortcutAumid' -as [type])) {
    Add-Type -TypeDefinition $CSharpHere  # the class above, in a here-string
}
[QMKonnect.ShortcutAumid]::Set($LnkPath, $Aumid) | Out-Null
exit 0   # ALWAYS exit 0 — non-fatal; never abort the installer
```

---

## The 4 most critical gotchas

1. **PROPVARIANT layout is the #1 failure cause.** Must be
   `[StructLayout(LayoutKind.Explicit)]` with `vt` at `[FieldOffset(0)]`,
   three reserved WORDs at `[FieldOffset(2/4/6)]`, and the pointer at
   `[FieldOffset(8)]`. `LayoutKind.Sequential` produces wrong padding on x64 →
   silent failure or access violation. Size must be 24 bytes (explicit pad at
   `[FieldOffset(16)]`). VT_LPWSTR = 31.
2. **`IPersistFile.Save` is REQUIRED.** `IPropertyStore.Commit` only flushes to
   the in-memory ShellLink object; it does NOT write the `.lnk` file (Microsoft
   docs: "SetValue affects the current property store instance only"). Omit
   `Save` → the AUMID is set in memory and lost when the COM object releases.
   Use `IsDirty()` then `Save(path, true)`.
3. **No `CoInitializeEx` needed from PowerShell.** PowerShell 5.1 runs STA and
   auto-initializes COM. Calling `CoInitializeEx` manually risks
   `RPC_E_CHANGED_MODE`. No elevation needed — per-user `.lnk` files live under
   `%APPDATA%\Microsoft\Windows\Start Menu\Programs\` (user-writable).
4. **`Add-Type` re-run guard is mandatory.** `Add-Type` compiles into the
   AppDomain; a second `Add-Type` for an already-defined type throws. Guard with
   `if (-not ('QMKonnect.ShortcutAumid' -as [type])) { Add-Type … }`.

## Verification — read the AUMID back from the `.lnk`

```powershell
$lnk = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\QMKonnect.lnk"
# (a) Quick check via the Shell COM property-store read:
Add-Type -TypeDefinition $CSharpHere   # includes a Get() helper (see PRP Task 1)
[QMKonnect.ShortcutAumid]::Get($lnk)   # → Mulletware.QMKonnect

# (b) Altern: PowerShell extended-property read (no C#):
(New-Object -ComObject Shell.Application).NameSpace(0).Items() | Out-Null  # warm up
# Most reliable is the C# Get(); the WindowsExtendedProperties path is flaky.
```

## Canonical references

- Send a local toast from desktop (Win32): https://learn.microsoft.com/en-us/windows/apps/design/shell/tiles-and-notifications/send-local-toast-desktop-cpp-wrl
  → establishes the "AUMID + Start Menu shortcut with System.AppUserModel.ID" prerequisite and the IPropertyStore/IPersistFile sequence.
- `IPropertyStore::SetValue`: https://learn.microsoft.com/en-us/windows/win32/api/propsys/nf-propsys-ipropertystore-setvalue
  → "affects the current property store instance only" (why Save is needed).
- `System.AppUserModel.ID` property (PKEY): https://github.com/MicrosoftDocs/win32/blob/docs/desktop-src/properties/props-system-appusermodel-id.md
  → PKEY_AppUserModel_ID = `{9F4C2855-…}`, pid 5; PropVariant type VT_LPWSTR.
- C# reference impl (handle shortcut with AppUserModelID): https://emoacht.wordpress.com/2012/11/14/csharp-appusermodelid/
- `IShellLinkW` / `IPersistFile`: https://learn.microsoft.com/en-us/windows/win32/shell/links