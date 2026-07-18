#![cfg(target_os = "linux")]
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Path of the on-demand, config-driven fallback udev rule. Only written when a
/// user sets VID/PID (to disambiguate among multiple QMK boards, or target a
/// custom one). Default keyboards are covered by the static usage-page rule.
const RULES_PATH: &str = "/etc/udev/rules.d/99-qmkonnect.rules";

/// Render the config-driven VID/PID fallback udev rule. Returns `None` when
/// both IDs are `None` — in that case the static usage-page rule
/// (`69-qmkonnect-rawhid.rules`) already grants permissions to any 0xFF60/0x61
/// device, so no per-device fallback is needed.
///
/// When exactly one of VID/PID is set, only that `ATTRS{...}` clause is emitted:
/// udev `ATTRS=="..."` cannot wildcard, so the unset side is omitted entirely
/// (the rule then matches any value for it) rather than emitting an impossible
/// `=="*"`. Shared by [`update_udev_rules`] (`qmkonnect --reload`) and the
/// Linux Settings dialog (pkexec install).
///
/// # ⚠️ udev line semantics
///
/// This MUST render to **exactly one udev rule line**. udev (see `man udev`,
/// systemd 261) treats every newline as the end of a rule — only a trailing
/// backslash `\` continues a line, **a trailing comma does not**. So a line
/// that begins with an *assignment* key (`GROUP=`/`MODE=`/`TAG+=`/`SYMLINK+=`/
/// `ENV{…}=`) and no preceding match key matches **every device on the host**
/// and silently corrupts host-wide permissions (see
/// `BUG_linux_udev_global_device_permissions.md`).
///
/// The Rust string uses line-continuations (`\` at end of a source line) only
/// to keep the source readable — they are NOT emitted; the rendered rule is one
/// physical line beginning with the `KERNEL==` match key. The regression tests
/// below (`render_rule_is_a_single_safe_line_*`) fail loudly if anyone
/// reformats this back into the dangerous multi-line form.
pub fn render_vidpid_rule(vendor_id: Option<u16>, product_id: Option<u16>) -> Option<String> {
    if vendor_id.is_none() && product_id.is_none() {
        return None;
    }
    let mut attrs = String::new();
    if let Some(v) = vendor_id {
        attrs.push_str(&format!("ATTRS{{idVendor}}==\"{v:04x}\", "));
    }
    if let Some(p) = product_id {
        attrs.push_str(&format!("ATTRS{{idProduct}}==\"{p:04x}\", "));
    }
    Some(format!(
        "# Managed by qmkonnect --reload; edit config.toml then re-run to update.\n\
KERNEL==\"hidraw*\", SUBSYSTEM==\"hidraw\", {attrs}\
TAG+=\"uaccess\", GROUP=\"input\", MODE=\"0660\", SYMLINK+=\"qmkonnect_device\", \
TAG+=\"systemd\", ENV{{SYSTEMD_USER_WANTS}}+=\"qmkonnect.service\"\n"
    ))
}

// ---------------------------------------------------------------------------
// Legacy-rule repair (BUG_linux_udev_global_device_permissions.md)
// ---------------------------------------------------------------------------

/// Detect whether an on-disk udev rule is the **globally-dangerous**
/// "multi-line / bare-assignment" form written by older qmkonnect builds.
///
/// udev treats every newline as the end of a rule (only a trailing `\`
/// continues a line — a trailing comma does **not**). So any logical rule line
/// whose *first* key is an assignment (`GROUP=`/`MODE=`/`TAG+=`/`SYMLINK+=`/
/// `ENV{…}=`) rather than a match (`KERNEL==`/`SUBSYSTEM==`/`ATTRS{…}==`)
/// matches **every device on the host**, chowning them to `root:input 0660`
/// and breaking `/dev/null`, `/dev/kvm`, `/dev/fuse`, etc.
///
/// This joins `\`-continued lines first (a legitimately-continued rule stays a
/// single logical rule), then flags any remaining line that lacks a leading
/// match key.
pub(crate) fn is_rule_globally_dangerous(content: &str) -> bool {
    // Collapse backslash line-continuations (a trailing backslash followed by
    // a newline joins two physical lines into one logical rule). Handles both
    // LF and CRLF endings.
    let joined = content.replace("\\\n", " ").replace("\\\r\n", " ");
    joined.lines().any(|raw| {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            return false;
        }
        !rule_line_has_leading_match_key(line)
    })
}

/// True iff the *first* key of a udev rule line is a match key (`==`/`!=`),
/// i.e. the line is scoped to specific devices. A line whose first key is an
/// assignment (`=`/`+=`/`:=`/`-=`) has no match and therefore matches every
/// device — the globally-dangerous case.
///
/// Key names are `[A-Z_]+` optionally followed by `{...}` (e.g. `ATTRS{...}`,
/// `ENV{...}`, `IMPORT{...}`). The operator is whatever follows that.
fn rule_line_has_leading_match_key(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0;
    // Skip the key name: ASCII uppercase letters and underscores.
    while i < bytes.len() && (bytes[i].is_ascii_uppercase() || bytes[i] == b'_') {
        i += 1;
    }
    // Optional { ... } payload (ATTRS{...}, ENV{...}, IMPORT{...}, TEST{...}).
    if i < bytes.len() && bytes[i] == b'{' {
        while i < bytes.len() && bytes[i] != b'}' {
            i += 1;
        }
        if i < bytes.len() {
            i += 1; // consume the closing '}'
        }
    }
    // The operator immediately follows. A match key uses `==` or `!=`.
    line[i..].starts_with("==") || line[i..].starts_with("!=")
}

// Get configuration paths in order of preference
pub fn get_config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // Try XDG_CONFIG_HOME first (most standard). NOTE: an *empty* value
    // must be treated as unset, otherwise `PathBuf::from("").join(...)` is
    // a *relative* path and we'd write the config into the CWD. `dirs` itself
    // treats empty as unset, but `std::env::var` returns `Ok("")` for empty.
    if let Ok(xdg_config) = std::env::var("XDG_CONFIG_HOME") {
        if !xdg_config.is_empty() {
            paths.push(
                PathBuf::from(xdg_config)
                    .join("qmk-notifier")
                    .join("config.toml"),
            );
        }
    }

    // Try home directory paths as fallback
    if let Some(home) = dirs::home_dir() {
        paths.push(
            home.join(".config")
                .join("qmk-notifier")
                .join("config.toml"),
        );
    }

    // Try system-wide config as last resort
    paths.push(PathBuf::from("/etc/qmk-notifier/config.toml"));

    paths
}

/// Resolve the config file path for a reload, **root-aware**. Used instead of
/// the plain [`get_config_paths`] when we may be running as root (sudo/pkexec),
/// which is the heart of fixing #26: under `sudo`, `HOME=/root`, so the normal
/// search would never find the invoking user's `~/.config/qmk-notifier/config.toml`
/// and `reload_config` would silently no-op without writing any rule.
///
/// Resolution order:
///   1. explicit `--config <path>` wins;
///   2. when running as root, prefer the *invoking* user's config — resolving
///      their home from `--uid` / `--user` / `$SUDO_UID` / `$SUDO_USER` /
///      `$PKEXEC_UID` (via `getent passwd`), then a single-config `/home/*`
///      scan as a last resort;
///   3. the normal search path ([`get_config_paths`]);
///   4. **fail loudly** (never silently no-op) listing every path tried.
pub fn resolve_config_for_reload(
    explicit: Option<PathBuf>,
    user: Option<String>,
    uid: Option<u32>,
) -> Result<PathBuf, Box<dyn Error>> {
    let mut tried: Vec<PathBuf> = Vec::new();

    // 1. Explicit --config wins.
    if let Some(p) = explicit {
        if p.exists() {
            return Ok(p);
        }
        tried.push(p);
    }

    // 2. When root, prefer the INVOKING user's config (HOME is /root under sudo).
    let is_root = unsafe { libc::geteuid() } == 0;
    if is_root {
        // Resolve a target uid from: --uid > $SUDO_UID > $PKEXEC_UID.
        let target_uid = uid.or_else(|| {
            std::env::var("SUDO_UID")
                .ok()
                .and_then(|s| s.parse().ok())
                .or_else(|| {
                    std::env::var("PKEXEC_UID")
                        .ok()
                        .and_then(|s| s.parse().ok())
                })
        });
        // Resolve a target user name from: --user > $SUDO_USER.
        let target_user = user.or_else(|| std::env::var("SUDO_USER").ok());

        for home in resolve_homes(target_uid, target_user) {
            let p = home.join(".config/qmk-notifier/config.toml");
            if p.exists() {
                return Ok(p);
            }
            tried.push(p);
        }

        // Last resort: scan /home/* for a config (exactly one expected).
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/home") {
            for e in entries.flatten() {
                let p = e.path().join(".config/qmk-notifier/config.toml");
                if p.exists() {
                    found.push(p);
                }
            }
        }
        match found.len() {
            0 => {}
            1 => return Ok(found.into_iter().next().unwrap()),
            _ => tried.extend(found),
        }
    }

    // 3. Non-root (or root with nothing found): the normal search path.
    for p in get_config_paths() {
        if p.exists() {
            return Ok(p);
        }
        tried.push(p);
    }

    // 4. FAIL LOUDLY — never silently no-op (this is the heart of #26).
    let listing = tried
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");
    Err(format!(
        "No QMKonnect config found. Tried:\n{listing}\n\
         Run as your normal user, or `sudo -E qmkonnect -r`, or pass \
         `--config <path>` / `--user <name>` / `--uid <n>`."
    )
    .into())
}

/// Home directories for the given uid/user, looked up via `getent passwd`
/// (always present on Linux, no unsafe). Falls back to `/home/<user>` when
/// `getent` is unavailable or finds nothing. Deduped, uid-first.
fn resolve_homes(uid: Option<u32>, user: Option<String>) -> Vec<PathBuf> {
    let mut homes: Vec<PathBuf> = Vec::new();

    if let Some(u) = uid {
        if let Some(h) = getent_home(&u.to_string()) {
            push_unique(&mut homes, h);
        }
    }
    if let Some(name) = user.as_deref() {
        match getent_home(name) {
            Some(h) => push_unique(&mut homes, h),
            // getent unavailable / user unknown: best-effort /home/<user>.
            None => push_unique(&mut homes, PathBuf::from("/home").join(name)),
        }
    }

    homes
}

/// Look up a user's home directory (passwd field 6) via `getent passwd <key>`,
/// where `key` is a uid or username. Returns `None` on any failure.
fn getent_home(key: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("getent")
        .args(["passwd", key])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|l| l.split(':').nth(5))
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

fn push_unique(homes: &mut Vec<PathBuf>, p: PathBuf) {
    if !homes.contains(&p) {
        homes.push(p);
    }
}

/// Install the config-driven VID/PID fallback udev rule (issues #4, #26), and
/// **repair** a globally-dangerous legacy rule left by an older qmkonnect build
/// (BUG_linux_udev_global_device_permissions.md).
///
/// * No world-writable `/tmp` staging path + `sudo mv` race.
/// * No `sudo` invocation (which fails from systemd/GUI contexts without a TTY).
/// * Writes atomically in the rules directory when running as root (e.g. package
///   install or `sudo qmkonnect -r`); otherwise prints the exact rule and the
///   command to install it instead of failing silently.
/// * When both IDs are `None`, does nothing — the static usage-page rule covers
///   default keyboards — *unless* a dangerous legacy rule is still on disk, in
///   which case it is purged (host-wide device permissions stay corrupted across
///   reboots otherwise). The caller ([`reload_udev_rules`]) re-applies the fix.
pub fn update_udev_rules(
    vendor_id: Option<u16>,
    product_id: Option<u16>,
    verbose: bool,
) -> Result<(), Box<dyn Error>> {
    let path = Path::new(RULES_PATH);
    let existing = fs::read_to_string(path).ok();
    let dangerous = existing.as_deref().is_some_and(is_rule_globally_dangerous);

    let rule = match render_vidpid_rule(vendor_id, product_id) {
        Some(r) => r,
        None => {
            // No config-driven rule is needed. But if a *dangerous* legacy rule
            // is still on disk from an older build, purge it — otherwise every
            // device on the host keeps its corrupted permissions across reboots.
            if dangerous {
                purge_rule(path)?;
            } else if verbose {
                println!(
                    "VID/PID both unset — no config-driven udev rule needed;\n\
                     the static usage-page rule (69-qmkonnect-rawhid.rules) covers default QMK keyboards."
                );
            }
            return Ok(());
        }
    };

    // We have a fresh, correct rule to write. If the on-disk rule is the
    // globally-dangerous multi-line form from an older build, note that we're
    // repairing it — the overwrite below fixes it and the caller's
    // reload_udev_rules() re-applies it. Printed unconditionally (critical).
    if dangerous {
        println!(
            "Repairing globally-dangerous legacy udev rule at {} — overwriting \n\
             with the correct single-line form (a multi-line/assignment-only rule \n\
             re-permissions every device on the host).",
            path.display()
        );
    } else if verbose {
        println!("Updating udev rule at {}", path.display());
    }

    write_rule_atomic(path, &rule, verbose)
}

/// Atomically write a udev rule file in its target directory (root context).
/// On `PermissionDenied` (non-root) we don't attempt `sudo`; instead we print
/// the exact rule and a copy-paste install command, mirroring the
/// non-interactive contexts (systemd/GUI) this runs from.
fn write_rule_atomic(path: &Path, rule: &str, verbose: bool) -> Result<(), Box<dyn Error>> {
    let dir = path.parent().unwrap_or(Path::new("/etc/udev/rules.d"));

    match tempfile::NamedTempFile::new_in(dir) {
        Ok(mut tmp) => {
            tmp.write_all(rule.as_bytes())?;
            tmp.as_file().sync_all()?;
            if let Err(persist_err) = tmp.persist(path) {
                return Err(persist_err.error.into());
            }
            if verbose {
                println!("Updated udev rules at {}", path.display());
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            println!(
                "Not running as root; to install the udev rule, run:\n\
                 \n  sudo tee {rules_path} <<'EOF'\n{rule}EOF\n\
                 \nThen apply: sudo udevadm control --reload-rules && sudo udevadm trigger",
                rules_path = path.display(),
            );
        }
        Err(e) => return Err(e.into()),
    }
    Ok(())
}

/// Remove a globally-dangerous legacy udev rule. Best-effort on permission:
/// non-root callers get a copy-paste `sudo rm … && udevadm …` command instead
/// of an error. A missing file is not an error.
fn purge_rule(path: &Path) -> Result<(), Box<dyn Error>> {
    println!(
        "Removing globally-dangerous legacy udev rule at {} — a multi-line /\n\
         assignment-only rule re-permissions every device on the host\n\
         (e.g. /dev/null, /dev/kvm, /dev/fuse).",
        path.display()
    );
    match fs::remove_file(path) {
        Ok(()) => {
            println!("Removed {}", path.display());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            println!(
                "Not running as root; to remove the dangerous legacy rule, run:\n\
                 \n  sudo rm {rules_path} && sudo udevadm control --reload-rules && sudo udevadm trigger",
                rules_path = path.display(),
            );
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Reload udev rules. Runs `udevadm` directly (no `sudo`): it succeeds when run
/// as root (package install / `sudo qmkonnect -r`) and otherwise the caller logs
/// a non-fatal warning.
pub fn reload_udev_rules() -> Result<(), Box<dyn Error>> {
    let output = std::process::Command::new("udevadm")
        .args(["control", "--reload-rules"])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Failed to reload udev rules: {}", stderr).into());
    }

    Ok(())
}

// For creating configuration directory
pub fn create_config_dir() -> Result<PathBuf, Box<dyn Error>> {
    let config_dir = match std::env::var("XDG_CONFIG_HOME") {
        // Treat an empty XDG_CONFIG_HOME as unset — an empty value would make
        // PathBuf::from("").join(...) a *relative* path (CWD), not $HOME/.config.
        Ok(xdg_config) if !xdg_config.is_empty() => PathBuf::from(xdg_config).join("qmk-notifier"),
        _ => match dirs::home_dir() {
            Some(home) => home.join(".config").join("qmk-notifier"),
            None => return Err("Could not determine configuration directory".into()),
        },
    };

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_rule_is_none_when_both_unset() {
        // No VID/PID -> no fallback rule (static usage-page rule covers it).
        assert!(render_vidpid_rule(None, None).is_none());
    }

    #[test]
    fn render_rule_emits_both_attrs_when_both_set() {
        let rule = render_vidpid_rule(Some(0xfeed), Some(0x1234)).unwrap();
        assert!(rule.contains("ATTRS{idVendor}==\"feed\""));
        assert!(rule.contains("ATTRS{idProduct}==\"1234\""));
        // Both permission mechanisms present (ACL + group/mode fallback).
        assert!(rule.contains("TAG+=\"uaccess\""));
        assert!(rule.contains("GROUP=\"input\""));
        assert!(rule.contains("MODE=\"0660\""));
    }

    #[test]
    fn render_rule_omits_unset_attr_no_wildcard() {
        // Only VID set: PID clause must be OMITTED entirely — udev ATTRS can't
        // wildcard, so there must be no `=="*"` and no idProduct clause.
        let rule = render_vidpid_rule(Some(0xfeed), None).unwrap();
        assert!(rule.contains("ATTRS{idVendor}==\"feed\""));
        assert!(!rule.contains("idProduct"));
        assert!(!rule.contains("\"*\""));

        // Only PID set: symmetric.
        let rule = render_vidpid_rule(None, Some(0x0001)).unwrap();
        assert!(rule.contains("ATTRS{idProduct}==\"0001\""));
        assert!(!rule.contains("idVendor"));
    }

    // -------------------------------------------------------------------------
    // Regression tests for BUG_linux_udev_global_device_permissions.md.
    //
    // udev treats every newline as the end of a rule (only a trailing `\`
    // continues a line — a trailing comma does NOT). So a multi-line template,
    // or any line whose first key is an assignment (GROUP=/MODE=/TAG+=/...),
    // matches EVERY device on the host and corrupts host-wide permissions.
    // These tests fail immediately if anyone reformats RULE_TEMPLATE back into
    // the dangerous multi-line form.
    // -------------------------------------------------------------------------

    /// The rendered rule must contain exactly one non-comment rule line.
    #[test]
    fn render_rule_is_a_single_safe_line_one_rule() {
        let rule = render_vidpid_rule(Some(0xfeed), Some(0x1234)).unwrap();
        let rule_lines: Vec<&str> = rule
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .collect();
        assert_eq!(
            rule_lines.len(),
            1,
            "rendered udev rule must be a single rule line, got {}:\n{rule}",
            rule_lines.len()
        );
    }

    /// That single line must START with a match key (KERNEL==/SUBSYSTEM==/...),
    /// never an assignment (GROUP=/MODE=/TAG+=/...). The fixture covers every
    /// VID/PID combination since each yields a different first-clause layout.
    #[test]
    fn render_rule_is_a_single_safe_line_leading_match_key() {
        for (vid, pid) in [
            (Some(0xfeedu16), Some(0x1234u16)),
            (Some(0xfeed), None),
            (None, Some(0x0001)),
        ] {
            let rule = render_vidpid_rule(vid, pid).unwrap();
            let body = rule
                .lines()
                .map(str::trim)
                .find(|l| !l.is_empty() && !l.starts_with('#'))
                .expect("a rule line exists");
            assert!(
                rule_line_has_leading_match_key(body),
                "rendered rule line must start with a match key, got: {body}"
            );
        }
    }

    /// Symmetry: the generated rule is NOT flagged as globally dangerous.
    #[test]
    fn generated_rule_is_not_globally_dangerous() {
        for (vid, pid) in [
            (Some(0xfeedu16), Some(0x1234u16)),
            (Some(0xfeed), None),
            (None, Some(0x0001)),
        ] {
            let rule = render_vidpid_rule(vid, pid).unwrap();
            assert!(
                !is_rule_globally_dangerous(&rule),
                "generated rule flagged dangerous:\n{rule}"
            );
        }
    }

    /// The exact broken multi-line form from the bug report MUST be detected.
    /// This is the regression sentinel: if `is_rule_globally_dangerous` ever
    /// stops catching this, every device on the host is at risk again.
    #[test]
    fn detects_broken_multiline_rule_from_bug_report() {
        let broken = "\
KERNEL==\"hidraw*\",
  ATTRS{idVendor}==\"feed\",
  ATTRS{idProduct}==\"0000\",
  GROUP=\"input\", MODE=\"0660\",
  TAG+=\"uaccess\",
  SYMLINK+=\"qmkonnect_device\",
  TAG+=\"systemd\", ENV{SYSTEMD_USER_WANTS}+=\"qmkonnect.service\"\n";
        assert!(
            is_rule_globally_dangerous(broken),
            "broken multi-line rule was NOT detected (regression!)"
        );
    }

    /// A correctly `\`-continued multi-line rule is safe (continuations join).
    #[test]
    fn safe_continued_multiline_rule_is_not_dangerous() {
        let safe = "\
KERNEL==\"hidraw*\", SUBSYSTEM==\"hidraw\", \\
  ATTRS{idVendor}==\"feed\", ATTRS{idProduct}==\"0000\", \\
  GROUP=\"input\", MODE=\"0660\", TAG+=\"uaccess\"\n";
        assert!(!is_rule_globally_dangerous(safe));
    }

    /// A comment-only / blank file is not dangerous.
    #[test]
    fn comment_only_file_is_not_dangerous() {
        assert!(!is_rule_globally_dangerous("# just a comment\n\n"));
    }

    /// The shipped static usage-page rule (69-qmkonnect-rawhid.rules) is safe.
    #[test]
    fn static_usage_page_rule_is_not_dangerous() {
        let static_rule =
            "SUBSYSTEM==\"hidraw\", IMPORT{program}=\"/usr/lib/udev/qmkonnect-hid-id %S%p\"\n\
ENV{ID_QMKONNECT}==\"1\", GROUP=\"input\", MODE=\"0660\", TAG+=\"uaccess\", \\
SYMLINK+=\"qmkonnect_device\", TAG+=\"systemd\", ENV{SYSTEMD_USER_WANTS}+=\"qmkonnect.service\"\n";
        assert!(!is_rule_globally_dangerous(static_rule));
    }
}
