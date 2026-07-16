//! `qmkonnect-hid-id` — udev helper that tags QMK Raw HID interfaces.
//!
//! Invoked by the static udev rule (`69-qmkonnect-rawhid.rules`) as
//!   qmkonnect-hid-id %S%p
//! i.e. with the hidraw syspath as argv[1]. It reads that interface's raw HID
//! report descriptor and, if it carries the QMK Raw HID signature (usage page
//! 0xFF60, usage 0x61), prints `ID_QMKONNECT=1` so the rule can grant
//! permissions uniformly to every such keyboard — no per-VID/PID config, no
//! `--reload`, no sudo for default users.
//!
//! Pure `std`: no hidapi, no heavy deps. Runs in udev context and must start
//! fast. Never fails loudly — udev treats no stdout as "no properties", so any
//! unreadable/truncated/unknown descriptor simply yields no match.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

/// QMK Raw HID usage page / usage (the stable firmware convention; overridable
/// in firmware via `RAW_USAGE_PAGE`/`RAW_USAGE_ID`, which is exactly the case
/// the static rule does NOT cover — those users fall back to the config-driven
/// rule via `qmkonnect --reload`).
const QMK_USAGE_PAGE: u32 = 0xFF60;
const QMK_USAGE: u32 = 0x61;

fn main() {
    process::exit(run());
}

fn run() -> i32 {
    let syspath = match resolve_syspath() {
        Some(p) => p,
        None => return 0, // nothing to inspect
    };

    // Each hidraw node exposes its parent HID device's raw report descriptor
    // under `device/`. Absent/unreadable => not a HID device we can classify.
    let descriptor_path = syspath.join("device").join("report_descriptor");
    let bytes = match fs::read(&descriptor_path) {
        Ok(b) => b,
        Err(_) => return 0, // unreadable -> no match (udev sees no properties)
    };

    if matches_qmk_signature(&bytes) {
        // The sole property the static rule keys off. `IMPORT{program}` consumes
        // `KEY=value` lines from stdout; a trailing newline is conventional.
        println!("ID_QMKONNECT=1");
    }

    0
}

/// Resolve the hidraw syspath to inspect: argv[1] if given, else `$DEVPATH`
/// prefixed with `/sys` (how udev supplies the path absent the explicit arg).
fn resolve_syspath() -> Option<PathBuf> {
    let mut args = env::args_os();
    let _ = args.next(); // program name
    if let Some(p) = args.next() {
        return Some(PathBuf::from(p));
    }
    env::var("DEVPATH")
        .ok()
        .filter(|s| !s.is_empty())
        .map(|d| syspath_from_devpath(&d))
}

/// Build the `/sys` syspath from a (possibly absolute) udev `$DEVPATH`.
/// udev sets DEVPATH absolute, e.g. `/devices/.../hidraw/hidraw4`; `Path::join`
/// would replace the `/sys` base with the absolute tail, so strip the leading
/// slash before joining.
fn syspath_from_devpath(devpath: &str) -> PathBuf {
    let trimmed = devpath.trim_start_matches('/');
    PathBuf::from("/sys").join(trimmed)
}

/// Walk the HID report descriptor item stream looking for the QMK Raw HID
/// signature: a Global Usage Page item set to 0xFF60 and a Local Usage item set
/// to 0x61 (the page need only be set before the usage appears; items in
/// between are ignored). Returns true on the first such match.
///
/// HID short-item layout (one prefix byte): `bSize` = bits 0-1 (data size:
/// 0,1,2,4 bytes; `3` means 4), `bType` = bits 2-3 (0=Main,1=Global,2=Local),
/// `bTag` = bits 4-7. A `0xFE` prefix introduces a long item.
fn matches_qmk_signature(buf: &[u8]) -> bool {
    let mut current_usage_page: u32 = 0;
    let mut i = 0;

    while i < buf.len() {
        let prefix = buf[i];

        // Long item: [0xFE][bDataSize][bLongItemTag][data...]. We don't care
        // about its contents, just skip past it.
        if prefix == 0xFE {
            let data_size = match buf.get(i + 1) {
                Some(&s) => s as usize,
                None => return false, // truncated long-item header
            };
            i = match i.checked_add(3).and_then(|x| x.checked_add(data_size)) {
                Some(x) => x,
                None => return false,
            };
            continue;
        }

        // Short item.
        let size = match prefix & 0x03 {
            0 => 0,
            1 => 1,
            2 => 2,
            3 => 4,
            _ => unreachable!(),
        };
        let btype = (prefix >> 2) & 0x03;
        let btag = (prefix >> 4) & 0x0F;

        // Bounds-check the data bytes for this item; truncation => give up.
        let data_start = i + 1;
        let data_end = match data_start.checked_add(size) {
            Some(e) if e <= buf.len() => e,
            _ => return false,
        };
        let data = &buf[data_start..data_end];

        match (btype, btag) {
            // Global Usage Page.
            (1, 0) => current_usage_page = read_le(data),
            // Local Usage. (HID spec: Local item tag 0 = Usage. The verified
            // byte signature `09 61` decodes to bType=2, bTag=0 — i.e. tag 0,
            // not 2; tag 2 would be Usage Maximum and would emit `29 61`.)
            (2, 0) => {
                let usage = read_le(data);
                if current_usage_page == QMK_USAGE_PAGE && usage == QMK_USAGE {
                    return true;
                }
            }
            _ => {}
        }

        i = data_end;
    }

    false
}

/// Read 0–4 data bytes as a little-endian unsigned value (HID item data order).
fn read_le(data: &[u8]) -> u32 {
    let mut v: u32 = 0;
    for (shift, &b) in data.iter().take(4).enumerate() {
        v |= (b as u32) << (shift * 8);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_known_signature() {
        // `06 60 ff` = Global Usage Page 0xFF60 ; `09 61` = Local Usage 0x61.
        assert!(matches_qmk_signature(&[0x06, 0x60, 0xff, 0x09, 0x61]));
    }

    #[test]
    fn detects_signature_with_items_in_between() {
        // The page is set first; arbitrary items may follow before the usage.
        // 0x85 = Report ID (global, tag 8), 1 data byte; 0x75 = Report Size...
        assert!(matches_qmk_signature(&[
            0x06, 0x60, 0xff, // Usage Page 0xFF60
            0x85, 0x01, // Report ID 1 (noise)
            0x75, 0x08, // Report Size 8 (noise)
            0x09, 0x61, // Usage 0x61 -> match
        ]));
    }

    #[test]
    fn ignores_wrong_usage_page() {
        // Usage page 0xFF61 (one off) then usage 0x61 -> no match.
        assert!(!matches_qmk_signature(&[0x06, 0x61, 0xff, 0x09, 0x61]));
    }

    #[test]
    fn ignores_wrong_usage() {
        assert!(!matches_qmk_signature(&[0x06, 0x60, 0xff, 0x09, 0x62]));
    }

    #[test]
    fn ignores_usage_without_qmk_page() {
        // Usage 0x61 but under the generic-desktop usage page (0x0001) -> no match.
        assert!(!matches_qmk_signature(&[0x06, 0x01, 0x00, 0x09, 0x61]));
    }

    #[test]
    fn truncated_descriptor_is_no_match() {
        // Usage-page item declares 2 data bytes but only 1 present.
        assert!(!matches_qmk_signature(&[0x06, 0x60]));
    }

    #[test]
    fn empty_descriptor_is_no_match() {
        assert!(!matches_qmk_signature(&[]));
    }

    #[test]
    fn long_item_is_skipped_without_panic() {
        // A long item (0xFE, size 2, tag, 2 data bytes) before the signature.
        assert!(matches_qmk_signature(&[
            0xFE, 0x02, 0xAA, 0x11, 0x22, // long item (skipped)
            0x06, 0x60, 0xff, // Usage Page 0xFF60
            0x09, 0x61, // Usage 0x61 -> match
        ]));
    }

    #[test]
    fn read_le_is_little_endian() {
        assert_eq!(read_le(&[]), 0);
        assert_eq!(read_le(&[0x61]), 0x61);
        assert_eq!(read_le(&[0x60, 0xff]), 0xff60);
        assert_eq!(read_le(&[0x01, 0x02, 0x03, 0x04]), 0x04030201);
    }

    #[test]
    fn syspath_from_absolute_devpath() {
        // udev's $DEVPATH is absolute ("/devices/..."); the /sys prefix must be
        // added without the absolute tail replacing the base.
        assert_eq!(
            syspath_from_devpath("/devices/foo/hidraw/hidraw9"),
            PathBuf::from("/sys/devices/foo/hidraw/hidraw9")
        );
        // A stray relative devpath still joins sensibly.
        assert_eq!(
            syspath_from_devpath("devices/x"),
            PathBuf::from("/sys/devices/x")
        );
    }
}
