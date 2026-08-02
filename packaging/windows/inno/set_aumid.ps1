<#
.SYNOPSIS
    Sets System.AppUserModel.ID on an existing Start Menu .lnk (toast prerequisite).
.DESCRIPTION
    Opens an existing shortcut via IPersistFile, sets its AUMID via IPropertyStore,
    commits, and persists via IPersistFile.Save. Used by the Inno installer
    (QMKonnect.iss CurStepChanged) and the dev-loop install.ps1 so WinRT toasts
    render as "QMKonnect" instead of being silently suppressed. The .lnk must already
    exist (created by the installer's [Icons] section / install.ps1's WScript.Shell).
    ALWAYS exits 0 - AUMID is notification-branding only; never abort the install.
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

    // PROPVARIANT - LayoutKind.Explicit is MANDATORY (LayoutKind.Sequential = silent fail / AV).
    // VARTYPE at byte 0, three reserved WORDs (bytes 2..7), union pointer at byte 8.
    // 24 bytes on x64 (pad at byte 16). VT_LPWSTR = 31.
    [StructLayout(LayoutKind.Explicit)]
    internal struct PropVariant {
        [FieldOffset(0)]  public ushort vt;
        [FieldOffset(2)]  public ushort wReserved1;
        [FieldOffset(4)]  public ushort wReserved2;
        [FieldOffset(6)]  public ushort wReserved3;
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
                // The ShellLink CoClass implements both interfaces - the cast triggers QI via the RCW.
                var ps = (IPropertyStore)obj;
                var pv = PropVariant.FromString(aumid);
                try {
                    if (ps.SetValue(ref PKEY_AppUserModel_ID, ref pv) != 0)
                        return unchecked((int)0x80004005);
                    ps.Commit();
                } finally { pv.Clear(); }
                // Commit flushes in-memory only - persist the .lnk to disk.
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