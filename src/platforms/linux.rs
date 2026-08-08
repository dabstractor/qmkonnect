#![cfg(target_os = "linux")]
// The `wayland`/`gnome`/`atspi` features don't exist yet (they land in
// P2.M1.T2.S2). Their `#[cfg(feature = "…")]` candidate rows + probe stubs are
// therefore simply not compiled today; each future backend task (P2.M2/M3/M4)
// replaces its stub. Silence the check-cfg warning for the not-yet-defined
// feature values here (the proper Cargo.toml `[lints]` check-cfg declaration
// is part of T2.S2's Cargo.toml edit, which owns the feature definitions).
#![allow(unexpected_cfgs)]
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::platforms::WindowMonitor;

// ---------------------------------------------------------------------------
// Runtime backend selector (PLATFORMS.md §6) — replaces the former compile-time
// `cfg(feature="hyprland")` either/or in `create_monitor`. Probes each
// compiled-in backend in a fixed priority order and returns the first present
// one; an optional forced override errors LOUDLY (with every probe result) when
// unavailable. On `Err` the runner keeps the tray + device pipeline alive
// (PLATFORMS.md §6 "No-backend fallback"); the GNOME one-shot notify fires
// there (see runners/linux.rs).
// ---------------------------------------------------------------------------

/// A probe function: `Ok` = this backend is available right now, `Err(reason)` =
/// why not. Plain `fn` pointer (not a closure) so the candidate list needs no
/// lifetimes / captures.
type ProbeFn = fn(verbose: bool) -> Result<(), String>;

/// A compiled-in backend candidate: its name + its availability probe.
/// Construction is a SEPARATE match (see [`construct_backend`]) so a stub probe
/// never has to name an unwritten backend type.
struct BackendCandidate {
    name: &'static str,
    probe: ProbeFn,
}

/// Priority-ordered list of compiled-in backends (PLATFORMS.md §6). Each
/// `#[cfg(feature = "…")]` row is simply absent when its feature is off; the
/// feature-undefined stub probes (wayland/gnome/atspi) are not compiled today.
/// X11 is always present and ALWAYS LAST (lowest priority; never under Wayland
/// via its own probe gate).
#[cfg(target_os = "linux")]
fn linux_backend_candidates() -> Vec<BackendCandidate> {
    [
        #[cfg(feature = "wayland")]
        ("foreign-toplevel", wayland_probe as ProbeFn),
        #[cfg(feature = "gnome")]
        ("gnome", gnome_probe as ProbeFn),
        #[cfg(feature = "hyprland")]
        (
            "hyprland",
            crate::platforms::hyprland::probe_available as ProbeFn,
        ),
        #[cfg(feature = "atspi")]
        ("atspi", atspi_probe as ProbeFn),
        // X11 is unconditional on Linux (always last — lowest priority; never
        // under Wayland via its own probe).
        ("x11", crate::platforms::x11::probe_available as ProbeFn),
    ]
    .into_iter()
    .map(|(name, probe)| BackendCandidate { name, probe })
    .collect()
}

/// Construct the chosen backend's monitor. Only backends that actually exist
/// have arms; a not-yet-wired name (a stub whose probe somehow returned `Ok`)
/// hits the catch-all `Err`. Kept separate from the probe list so stub rows
/// never reference unwritten backend types (no breakage when the features land
/// in P2.M1.T2.S2 before the backends in P2.M2/M3/M4).
fn construct_backend(name: &str, verbose: bool) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    match name {
        #[cfg(feature = "wayland")]
        "foreign-toplevel" => Ok(Box::new(
            crate::platforms::wayland_ft::WaylandFtMonitor::new(verbose),
        )),
        #[cfg(feature = "gnome")]
        "gnome" => Ok(Box::new(crate::platforms::gnome::GnomeMonitor::new(
            verbose,
        ))),
        #[cfg(feature = "hyprland")]
        "hyprland" => Ok(Box::new(crate::platforms::hyprland::HyprlandMonitor::new(
            verbose,
        ))),
        #[cfg(feature = "atspi")]
        "atspi" => Ok(Box::new(crate::platforms::atspi::AtspiMonitor::new(
            verbose,
        ))),
        "x11" => Ok(Box::new(crate::platforms::x11::X11Monitor::new(verbose))),
        other => Err(format!(
            "backend '{other}' was selected but its construction is not wired in this build"
        )
        .into()),
    }
}

/// Runtime Linux backend selector (PLATFORMS.md §6). `forced` overrides the
/// priority order (default `auto`); a forced backend that is unavailable errors
/// LOUDLY with every probe result. `None` ⇒ auto first-available.
///
/// This is what `create_monitor` delegates to on Linux; the `[linux] backend`
/// config value is wired into `forced` by P2.M1.T2.S1 (today `create_monitor`
/// passes `None`).
pub fn select_linux_backend(
    verbose: bool,
    forced: Option<&str>,
) -> Result<Box<dyn WindowMonitor>, Box<dyn Error>> {
    let candidates = linux_backend_candidates();
    let names: Vec<&str> = candidates.iter().map(|c| c.name).collect();

    if let Some(want) = forced {
        if verbose {
            println!("select_linux_backend: forced backend '{want}'");
        }
        // Loud diagnostic: run EVERY probe so the user sees why the forced one
        // failed (PLATFORMS.md §6: "errors loudly with every probe result").
        let mut diag: Vec<String> = Vec::new();
        for c in &candidates {
            let r = (c.probe)(verbose);
            diag.push(format!(
                "  {}: {}",
                c.name,
                r.as_ref().err().map(|e| e.as_str()).unwrap_or("available")
            ));
        }
        match candidates.iter().find(|c| c.name == want) {
            Some(c) => match (c.probe)(verbose) {
                Ok(()) => construct_backend(want, verbose),
                Err(reason) => Err(format!(
                    "forced backend '{want}' is unavailable ({reason}). Every probe result:\n{}",
                    diag.join("\n")
                )
                .into()),
            },
            None => Err(format!(
                "forced backend '{want}' is not compiled into this binary (compiled-in: [{}])",
                names.join(", ")
            )
            .into()),
        }
    } else {
        for c in &candidates {
            if verbose {
                println!("select_linux_backend: probing '{}'…", c.name);
            }
            match (c.probe)(verbose) {
                Ok(()) => {
                    if verbose {
                        println!("  → '{name}' available, selected", name = c.name);
                    }
                    return construct_backend(c.name, verbose);
                }
                Err(reason) => {
                    if verbose {
                        println!("  → '{name}' unavailable: {reason}", name = c.name);
                    }
                }
            }
        }
        Err(format!(
            "no Linux window backend available (probed: [{}])",
            names.join(", ")
        )
        .into())
    }
}

// Feature-gated probe STUBS for the backends that don't exist yet (P2.M2/M3/M4).
// These are SELF-CONTAINED (no external-module refs) so that adding the features
// in P2.M1.T2.S2 does NOT break the build before the real backends land. Each
// future backend task REPLACES its stub with a real probe + adds a
// `construct_backend` arm. Undefined features today ⇒ these are not compiled.
#[cfg(feature = "wayland")]
fn wayland_probe(_verbose: bool) -> Result<(), String> {
    crate::platforms::wayland_ft::probe_available(_verbose)
}
#[cfg(feature = "gnome")]
fn gnome_probe(_verbose: bool) -> Result<(), String> {
    crate::platforms::gnome::probe_available(_verbose)
}
#[cfg(feature = "atspi")]
fn atspi_probe(verbose: bool) -> Result<(), String> {
    crate::platforms::atspi::probe_available(verbose)
}

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
                    .join("qmkonnect")
                    .join("config.toml"),
            );
        }
    }

    // Try home directory paths as fallback
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".config").join("qmkonnect").join("config.toml"));
    }

    // Try system-wide config as last resort
    paths.push(PathBuf::from("/etc/qmkonnect/config.toml"));

    paths
}

/// Resolve the config file path for a reload, **root-aware**. Used instead of
/// the plain [`get_config_paths`] when we may be running as root (sudo/pkexec),
/// which is the heart of fixing #26: under `sudo`, `HOME=/root`, so the normal
/// search would never find the invoking user's `~/.config/qmkonnect/config.toml`
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
            let p = home.join(".config/qmkonnect/config.toml");
            if p.exists() {
                return Ok(p);
            }
            tried.push(p);
        }

        // Last resort: scan /home/* for a config (exactly one expected).
        let mut found = Vec::new();
        if let Ok(entries) = std::fs::read_dir("/home") {
            for e in entries.flatten() {
                let p = e.path().join(".config/qmkonnect/config.toml");
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
        Ok(xdg_config) if !xdg_config.is_empty() => PathBuf::from(xdg_config).join("qmkonnect"),
        _ => match dirs::home_dir() {
            Some(home) => home.join(".config").join("qmkonnect"),
            None => return Err("Could not determine configuration directory".into()),
        },
    };

    fs::create_dir_all(&config_dir)?;
    Ok(config_dir)
}

/// First-run, default-on login autostart for binary-only installs (Scoop /
/// cargo-binstall / generic tarball) that have no system-package `postinst` to
/// install `/etc/xdg/autostart/qmkonnect.desktop`. Writes the **user**
/// equivalent at `$XDG_CONFIG_HOME/autostart/qmkonnect.desktop` (or
/// `~/.config/autostart/…`) with `Exec = <absolute current_exe()>`, gated by a
/// marker file so it never re-enables after the user removes it — mirrors the
/// macOS first-run default-on (`UI.md` §6.2 / `LINUX.md` §6.3). Idempotent; a
/// user file shadows a same-named system file in the XDG spec, so this is safe
/// alongside a packaged install (no double-launch). Failures are non-fatal.
pub fn ensure_xdg_autostart(verbose: bool) {
    // 1. Marker file in the qmkonnect config dir — skip if already initialized.
    //    Never fight the user: if they deleted the .desktop, we don't recreate it.
    let config_dir = match create_config_dir() {
        Ok(p) => p,
        Err(e) => {
            if verbose {
                eprintln!("ensure_xdg_autostart: no config dir: {e}");
            }
            return;
        }
    };
    let marker = config_dir.join(".autostart_initialized");
    if marker.exists() {
        return;
    }

    // 2. Resolve the user autostart dir (same XDG logic as create_config_dir:
    //    an *empty* XDG_CONFIG_HOME is treated as unset so we never build a
    //    relative path).
    let autostart_dir = match std::env::var("XDG_CONFIG_HOME") {
        Ok(x) if !x.is_empty() => PathBuf::from(x).join("autostart"),
        _ => match dirs::home_dir() {
            Some(home) => home.join(".config").join("autostart"),
            None => {
                if verbose {
                    eprintln!("ensure_xdg_autostart: cannot determine home dir");
                }
                return;
            }
        },
    };
    if let Err(e) = fs::create_dir_all(&autostart_dir) {
        if verbose {
            eprintln!(
                "ensure_xdg_autostart: cannot create {}: {e}",
                autostart_dir.display()
            );
        }
        return;
    }

    // 3. Exec = absolute current_exe (portable installs); fall back to the bare
    //    name on PATH for the rare case current_exe() is unavailable.
    let exec = std::env::current_exe()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "qmkonnect".to_string());

    let desktop = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=QMKonnect\n\
         Comment=Send the foreground window to your QMK keyboard\n\
         Exec={exec}\n\
         Icon=input-keyboard\n\
         Terminal=false\n\
         NoDisplay=true\n\
         X-GNOME-Autostart-enabled=true\n\
         Categories=Utility;\n"
    );

    let target = autostart_dir.join("qmkonnect.desktop");
    if let Err(e) = fs::write(&target, &desktop) {
        if verbose {
            eprintln!(
                "ensure_xdg_autostart: cannot write {}: {e}",
                target.display()
            );
        }
        return;
    }

    // 4. Touch the marker so we never rewrite the user's file (they own it now).
    let _ = fs::write(&marker, b"");
    if verbose {
        println!("Wrote login autostart entry: {}", target.display());
    }
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

// ---------------------------------------------------------------------------
// select_linux_backend + probe tests (PLATFORMS.md §6 / ARCHITECTURE.md §10).
//
// These manipulate PROCESS-GLOBAL env vars ($DISPLAY, $WAYLAND_DISPLAY,
// $HYPRLAND_INSTANCE_SIGNATURE, $XDG_RUNTIME_DIR), so the whole crate MUST
// run single-threaded (`cargo test --bin qmkonnect -- --test-threads=1`,
// Invariant 8). Each test restores the env it touched so a later test starts
// from a clean baseline.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod select_tests {
    use super::*;
    #[cfg(feature = "hyprland")]
    use std::os::unix::net::UnixListener;

    /// True iff `xprop` is on PATH. The X11 probe's third gate depends on it;
    /// tests that require xprop skip (early-pass) when it's absent so a CI box
    /// without xprop doesn't fail spuriously. Resolves the binary on PATH
    /// (rather than `xprop -version`, which needs a live $DISPLAY).
    fn xprop_present() -> bool {
        std::process::Command::new("which")
            .arg("xprop")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    /// Snapshot of an env var (or `None` if unset) for save/restore around a test.
    fn env_snapshot(key: &str) -> Option<Option<String>> {
        Some(std::env::var(key).ok())
    }

    /// Restore an env var to its snapshotted state.
    fn env_restore(key: &str, snap: Option<Option<String>>) {
        match snap {
            Some(Some(val)) => std::env::set_var(key, val),
            Some(None) => std::env::remove_var(key),
            None => {}
        }
    }

    // ---------------- x11 probe (Invariant 11 is the headline gate) ---------------

    #[test]
    fn x11_probe_err_when_display_unset() {
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        std::env::remove_var("DISPLAY");
        std::env::remove_var("WAYLAND_DISPLAY");
        let r = crate::platforms::x11::probe_available(false);
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        assert!(r.is_err(), "X11 probe must fail when $DISPLAY is unset");
        assert!(
            r.unwrap_err().contains("DISPLAY"),
            "the Err reason must name $DISPLAY"
        );
    }

    #[test]
    fn x11_probe_err_when_wayland_display_set() {
        // THE headline regression (Invariant 11): X11 is NEVER selected under a
        // Wayland compositor, even if $DISPLAY is set (XWayland sets it but
        // reports focus unreliably for native windows).
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        std::env::set_var("DISPLAY", ":0");
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        let r = crate::platforms::x11::probe_available(false);
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        assert!(
            r.is_err(),
            "X11 probe must fail under Wayland ($WAYLAND_DISPLAY set)"
        );
        let msg = r.unwrap_err();
        assert!(
            msg.contains("Wayland") || msg.contains("WAYLAND"),
            "the Err reason must mention the Wayland gate; got: {msg}"
        );
    }

    #[test]
    fn x11_probe_err_when_wayland_display_empty_treated_as_unset() {
        // An empty $WAYLAND_DISPLAY must NOT gate X11 off (treat Ok("") as unset,
        // matching get_config_paths()). Still needs xprop to reach Ok.
        if !xprop_present() {
            return;
        }
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        std::env::set_var("DISPLAY", ":0");
        std::env::set_var("WAYLAND_DISPLAY", "");
        let r = crate::platforms::x11::probe_available(false);
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        assert!(
            r.is_ok(),
            "empty $WAYLAND_DISPLAY must be treated as unset; got: {r:?}"
        );
    }

    #[test]
    fn x11_probe_ok_when_display_set_and_wayland_unset_with_xprop() {
        if !xprop_present() {
            return;
        }
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        std::env::set_var("DISPLAY", ":0");
        std::env::remove_var("WAYLAND_DISPLAY");
        let r = crate::platforms::x11::probe_available(false);
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        assert!(
            r.is_ok(),
            "X11 probe must pass with $DISPLAY set, no Wayland, xprop present"
        );
    }

    // ---------------- hyprland probe (hermetic: TempDir + UnixListener) ---------------

    #[cfg(feature = "hyprland")]
    #[test]
    fn hyprland_probe_err_when_no_signature() {
        let snap_s = env_snapshot("HYPRLAND_INSTANCE_SIGNATURE");
        let snap_r = env_snapshot("XDG_RUNTIME_DIR");
        std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");
        let r = crate::platforms::hyprland::probe_available(false);
        env_restore("HYPRLAND_INSTANCE_SIGNATURE", snap_s);
        env_restore("XDG_RUNTIME_DIR", snap_r);
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("HYPRLAND_INSTANCE_SIGNATURE"));
    }

    #[cfg(feature = "hyprland")]
    #[test]
    fn hyprland_probe_ok_with_a_live_socket() {
        // CLONE the existing hyprland_socket_is_live_* test shape: TempDir + a
        // bound UnixListener at $XDG_RUNTIME_DIR/hypr/<sig>/.socket.sock.
        let dir = tempfile::TempDir::new().expect("tempdir");
        let sig = "deadbeef";
        let hypr_dir = dir.path().join("hypr").join(sig);
        std::fs::create_dir_all(&hypr_dir).expect("mkdir hypr/<sig>");
        let socket = hypr_dir.join(".socket.sock");
        let _listener = UnixListener::bind(&socket).expect("bind socket");

        let snap_s = env_snapshot("HYPRLAND_INSTANCE_SIGNATURE");
        let snap_r = env_snapshot("XDG_RUNTIME_DIR");
        std::env::set_var("HYPRLAND_INSTANCE_SIGNATURE", sig);
        std::env::set_var("XDG_RUNTIME_DIR", dir.path());
        let r = crate::platforms::hyprland::probe_available(false);
        env_restore("HYPRLAND_INSTANCE_SIGNATURE", snap_s);
        env_restore("XDG_RUNTIME_DIR", snap_r);
        // keep _listener alive until here
        drop(_listener);
        assert!(r.is_ok(), "live socket ⇒ Ok; got: {r:?}");
    }

    // ---------------- select_linux_backend: forced + auto paths ---------------

    #[test]
    fn select_forced_unknown_backend_is_loud_err() {
        let r = select_linux_backend(false, Some("nonsense"));
        let msg = match r {
            Err(e) => e.to_string(),
            Ok(_) => panic!("unknown-forced backend must error, not construct a monitor"),
        };
        assert!(
            msg.contains("nonsense"),
            "unknown-forced err must name the requested backend; got: {msg}"
        );
        assert!(
            msg.contains("compiled-in"),
            "unknown-forced err must list the compiled-in backends; got: {msg}"
        );
    }

    #[test]
    fn select_forced_x11_under_wayland_is_loud_err() {
        // Forced-unavailable must error LOUDLY and include every probe result
        // (PLATFORMS.md §6). X11 under Wayland is unavailable (Invariant 11).
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        let snap_s = env_snapshot("HYPRLAND_INSTANCE_SIGNATURE");
        std::env::set_var("DISPLAY", ":0");
        std::env::set_var("WAYLAND_DISPLAY", "wayland-0");
        std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");
        let r = select_linux_backend(false, Some("x11"));
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        env_restore("HYPRLAND_INSTANCE_SIGNATURE", snap_s);
        assert!(r.is_err(), "forced X11 under Wayland must error loudly");
        let msg = match r {
            Err(e) => e.to_string(),
            Ok(_) => panic!("forced X11 under Wayland must error loudly"),
        };
        assert!(
            msg.contains("Every probe result"),
            "forced-unavailable err must list every probe result; got: {msg}"
        );
    }

    #[test]
    fn select_auto_picks_first_available() {
        // Craft env so a Linux backend is available (no Hyprland sig, DISPLAY
        // set, no Wayland, xprop present). The dispatcher must pick the
        // first available backend. NOTE: on a box with a real a11y bus
        // (`org.a11y.Bus` owned) the AT-SPI backend (priority #4) is selected
        // ahead of X11 (#5) — that is correct, not a failure. Before P2.M4.T1.S1
        // the atspi probe was a stub returning Err, so X11 was the only available;
        // now atspi is real, so this asserts "some compiled-in backend is picked"
        // rather than pinning X11 specifically.
        if !xprop_present() {
            return;
        }
        let snap_d = env_snapshot("DISPLAY");
        let snap_w = env_snapshot("WAYLAND_DISPLAY");
        let snap_s = env_snapshot("HYPRLAND_INSTANCE_SIGNATURE");
        let snap_r = env_snapshot("XDG_RUNTIME_DIR");
        let snap_a = env_snapshot("ATSPI_BUS_ADDRESS");
        std::env::set_var("DISPLAY", ":0");
        std::env::remove_var("WAYLAND_DISPLAY");
        std::env::remove_var("HYPRLAND_INSTANCE_SIGNATURE");
        std::env::remove_var("XDG_RUNTIME_DIR");
        std::env::remove_var("ATSPI_BUS_ADDRESS");
        let r = select_linux_backend(false, None);
        env_restore("DISPLAY", snap_d);
        env_restore("WAYLAND_DISPLAY", snap_w);
        env_restore("HYPRLAND_INSTANCE_SIGNATURE", snap_s);
        env_restore("XDG_RUNTIME_DIR", snap_r);
        env_restore("ATSPI_BUS_ADDRESS", snap_a);
        assert!(r.is_ok(), "auto must pick an available backend");
        // The selected backend must be a compiled-in Linux backend. With a real
        // a11y bus it's atspi (#4); without it (org.a11y.Bus unowned) it's X11.
        let monitor = r.unwrap();
        let name = monitor.platform_name();
        assert!(
            name == "atspi" || name == "Linux (X11)",
            "expected atspi or X11; got {name:?}"
        );
    }
}
