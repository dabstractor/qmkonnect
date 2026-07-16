//! Per-user autostart (Windows) backed by the HKCU `Run` key.
//!
//! Single source of truth shared with the installer
//! (`packaging/windows/install.ps1` / `uninstall.ps1`): value name `QMKonnect`
//! under `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`. The
//! tray "Open at Login" checkbox and the installer both manage this one value,
//! so they never desync (no double entries, no stale Startup-folder `.lnk`).
//!
//! Entirely `#[cfg(target_os = "windows")]` so it merges cleanly with the
//! parallel macOS SMAppService work (which lives privately inside `tray.rs`).
#![cfg(target_os = "windows")]

use windows::core::{w, PCWSTR};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RRF_RT_REG_SZ,
};

/// The per-user autostart subkey: Windows runs each `REG_SZ` value here on
/// login (this is the group Task Manager → Startup lists as "Run").
const SUBKEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
/// Value name. **This exact spelling is the contract shared with the
/// installer** — `install.ps1` writes it, `uninstall.ps1` deletes it.
const VALUE: PCWSTR = w!("QMKonnect");

/// Is the autostart entry currently present?
///
/// Presence-based: a Task-Manager "Disabled" override (a 12-byte value stored
/// separately under `StartupApproved\Run`) is intentionally *not* consulted —
/// most apps behave this way, and the registry is the truth the checkbox
/// reflects. See HANDOFF §5 "Task Manager Disabled desync".
pub fn is_enabled() -> bool {
    // A path fits easily; a `REG_SZ` path is bounded to ~520 bytes, so a 1 KiB
    // buffer never truncates. `RegGetValueW` fills `len` with the byte count on
    // both success and "more data", so a non-zero length + Ok means the value
    // exists and is readable.
    let mut buf = [0u8; 1024];
    let mut len = buf.len() as u32;
    let result = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            SUBKEY,
            VALUE,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut _),
            Some(&mut len),
        )
    };
    result.is_ok() && len > 0
}

/// Create (`enabled == true`) or remove (`enabled == false`) the autostart
/// entry, pointing at the running exe.
///
/// The path self-heals — it is whatever `current_exe()` resolves to at toggle
/// time — so moving the install and toggling once re-points the value. Registry
/// failures are swallowed; the caller re-derives the checkbox from
/// [`is_enabled`] and visibly reverts on failure.
pub fn set_enabled(enabled: bool) {
    if enabled {
        enable();
    } else {
        disable();
    }
}

fn enable() {
    let exe = current_exe_wide();
    let mut hkey = HKEY::default();
    // The `Run` key always exists, so this only fails on truly broken systems.
    let opened = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, SUBKEY, 0, KEY_SET_VALUE, &mut hkey) };
    if opened.is_err() {
        return;
    }
    // REG_SZ expects UTF-16 code units *including* the terminating NUL;
    // `current_exe_wide` already appends one, so its raw byte view is the data.
    let data: &[u8] =
        unsafe { std::slice::from_raw_parts(exe.as_ptr() as *const u8, exe.len() * 2) };
    let _ = unsafe { RegSetValueExW(hkey, VALUE, 0, REG_SZ, Some(data)) };
    let _ = unsafe { RegCloseKey(hkey) };
}

fn disable() {
    let mut hkey = HKEY::default();
    let opened = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, SUBKEY, 0, KEY_SET_VALUE, &mut hkey) };
    if opened.is_err() {
        return;
    }
    // Missing value is not an error for the caller's purpose (it's already
    // gone); discard the result.
    let _ = unsafe { RegDeleteValueW(hkey, VALUE) };
    let _ = unsafe { RegCloseKey(hkey) };
}

/// `std::env::current_exe()` as a null-terminated UTF-16 buffer — the layout
/// `REG_SZ` expects. Kept local rather than reusing `tray.rs`'s `to_wide_string`
/// so this module stays self-contained and the macOS branch merges cleanly.
fn current_exe_wide() -> Vec<u16> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;
    let path = match std::env::current_exe() {
        Ok(p) => p,
        Err(_) => return vec![0],
    };
    OsStr::new(&path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
