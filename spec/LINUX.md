# SPEC — Linux Integration (udev, systemd, SNI tray)

> Companion to `PRD.md` / `SPEC_PLATFORMS.md` / `SPEC_UI.md`. Everything
> Linux-specific that is *not* the Hyprland window monitor itself: the static
> udev rule + `qmkonnect-hid-id` helper, the config-driven fallback rule,
> dangerous-rule detection/repair, the root-aware `--reload`, the systemd user
> service, the SNI tray, and the GTK window-info dialog. Covers
> `src/platforms/linux.rs`, `src/bin/hid_id.rs`, `src/linux_tray.rs`, and
> `packaging/linux/`.

---

## 1. The Two-Rule Strategy ("hybrid")

Linux device permissions use **two complementary rules**, so nobody gets left out:

| Rule | File | Who it covers | When written |
|---|---|---|---|
| **Static usage-page rule** | `/usr/lib/udev/rules.d/69-qmkonnect-rawhid.rules` | **Every default QMK keyboard** (usage page `0xFF60` / usage `0x61`) | Shipped by the package; **never regenerated from config** |
| **Config-driven fallback** | `/etc/udev/rules.d/99-qmkonnect.rules` | Custom-usage/page users, or VID/PID disambiguation | Generated **on demand** by `qmkonnect --reload` / the Settings dialog |

The static rule is numbered **69** so it runs before any user-generated
`99-qmkonnect.rules`. Default users therefore need **no `--reload`, no sudo**.

---

## 2. The Static Rule

```
# packaging/linux/udev/69-qmkonnect-rawhid.rules
SUBSYSTEM=="hidraw", IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id %S%p"
ENV{ID_QMKONNECT}=="1", GROUP="input", MODE="0660", TAG+="uaccess", \
  SYMLINK+="qmkonnect_device", TAG+="systemd", \
  ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"
```

- **`IMPORT{program}`** runs `qmkonnect-hid-id` with the hidraw syspath (`%S%p`);
  it prints `ID_QMKONNECT=1` iff the interface carries the QMK signature (§3).
- **`ENV{ID_QMKONNECT}=="1"`** gates everything that follows (so non-matching
  devices are untouched).
- **Permissions:** `GROUP="input", MODE="0660"` (group-accessible hidraw node)
  **+** `TAG+="uaccess"` (per-session ACL via systemd-logind). `uaccess` is
  primary; the `GROUP`/`MODE` fallback is required because `uaccess` is applied
  once at device-add and is *not* retried — on a mid-session replug it can race
  logind and leave the node at the kernel default `0600 root`, locking out the
  app until reboot.
- **`SYMLINK+="qmkonnect_device"`** + **`TAG+="systemd"`** +
  **`ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"`** → systemd starts the user
  service when the device appears (and the `BindsTo` in the service stops it when
  the device disappears).

---

## 3. The `qmkonnect-hid-id` Helper (`src/bin/hid_id.rs`)

Pure **`std`** (no hidapi, no heavy deps — runs in udev context, must start
fast). Second bin target in `Cargo.toml`:
```toml
[[bin]]
name = "qmkonnect-hid-id"
path = "src/bin/hid_id.rs"
```

### 3.1 Behavior
- Resolve syspath: `argv[1]` if given (udev passes `%S%p`), else `$DEVPATH`
  prefixed with `/sys` (udev sets `DEVPATH` absolute, so strip the leading `/`
  before joining).
- Read `<syspath>/device/report_descriptor` (binary). Unreadable ⇒ exit 0
  printing nothing (udev treats no stdout as "no properties").
- Walk the HID report-descriptor item stream looking for the QMK signature: a
  **Global Usage Page** item (`bType==1, bTag==0`) set to `0xFF60`, followed by a
  **Local Usage** item (`bType==2, bTag==0`) set to `0x61`. Items between them
  are ignored.
- On match: print exactly `ID_QMKONNECT=1\n` and exit 0. No match ⇒ exit 0
  printing nothing.

### 3.2 HID item parsing (short + long items)
- Prefix byte `b`: `size = match b & 0x03 {0=>0,1=>1,2=>2,3=>4}`;
  `bType = (b>>2)&0x03`; `bTag = (b>>4)&0x0F`.
- `0xFE` prefix ⇒ **long item**: next byte is the data size; skip `2 + size`
  bytes (bounds-checked).
- Data read little-endian (`read_le`, 0–4 bytes).
- Bounds-check every item; truncation ⇒ exit 0 (no match), never panic.

### 3.3 Verified byte signatures (real hardware)
- Usage page `0xFF60` appears as `06 60 ff` (global, tag 0, 2 data bytes LE).
- Usage `0x61` appears as `09 61` (local, tag 0, 1 data byte). *(Tag 0, not 2 —
  tag 2 is Usage Maximum and would emit `29 61`.)*

---

## 4. The Config-Driven Fallback Rule + Root-Aware Reload (`src/platforms/linux.rs`)

### 4.1 `render_vidpid_rule(vendor_id, product_id) -> Option<String>`
- **`None`** when both IDs are `None` (the static rule already covers that case).
- Otherwise emits **exactly one physical rule line** beginning with the `KERNEL==`
  match key:
  ```
  # Managed by qmkonnect --reload; edit config.toml then re-run to update.
  KERNEL=="hidraw*", SUBSYSTEM=="hidraw", [ATTRS{idVendor}=="VVVV", ]\
  [ATTRS{idProduct}=="PPPP", ]\
  TAG+="uaccess", GROUP="input", MODE="0660", SYMLINK+="qmkonnect_device", \
  TAG+="systemd", ENV{SYSTEMD_USER_WANTS}+="qmkonnect.service"
  ```
- When only one of VID/PID is set, the unset `ATTRS{...}` clause is **omitted
  entirely** — udev `ATTRS=="..."` cannot wildcard (`=="*"` is invalid), so the
  unset side matches any value.

### 4.2 `update_udev_rules(vendor_id, product_id, verbose)` (used by `--reload`)
- Read the existing `/etc/udev/rules.d/99-qmkonnect.rules`; check if it's the
  globally-dangerous legacy form (§5).
- If both IDs unset: no fallback rule needed — *unless* a dangerous legacy rule
  is on disk, in which case **purge** it. Otherwise no-op (static rule covers it).
- If a rule is needed: write it **atomically** via `tempfile::NamedTempFile`
  in the rules dir + `sync_all` + `persist` (no predictable `/tmp` staging, no
  `sudo mv` race, no `sudo` invocation that fails without a TTY).
- On `PermissionDenied` (non-root): print the exact rule + a copy-paste
  `sudo tee … <<'EOF' … EOF` command instead of failing.

### 4.3 `reload_udev_rules()`
- Runs `udevadm control --reload-rules` directly (no `sudo`); succeeds as root,
  logs a non-fatal warning otherwise.

### 4.4 Root-aware config resolution (`resolve_config_for_reload`) — fixes #26
Under plain `sudo`, `HOME=/root`, so the normal search never finds the invoking
user's `~/.config/qmkonnect/config.toml` and the old code **silently
no-op'd** without writing the rule. New resolution order:
1. Explicit `--config <path>` wins.
2. **When root:** prefer the *invoking* user's config — resolve a target
   uid/name from `--uid` > `--user` > `$SUDO_UID` > `$PKEXEC_UID` (and
   `$SUDO_USER`), look up the home via **`getent passwd <key>`** (always
   present, no unsafe; field 6), then a last-resort single-config scan of
   `/home/*`.
3. The normal search path (`get_config_paths`).
4. **Fail loudly** — list every path tried and exit non-zero (never silently
   return `Ok`).

The reload CLI passes `--config`/`--user`/`--uid` (value flags parsed by
`main::parse_value_flag`).

---

## 5. Dangerous-Legacy-Rule Detection & Repair

> An older build wrote `/etc/udev/rules.d/99-qmkonnect.rules` as a **multi-line
> rule with no backslash continuations**. Because udev treats every newline as
> the end of a rule (a trailing comma does **not** continue a line), the
> bare-assignment lines matched **every device on the host** and re-permissioned
> them to `root:input 0660` — breaking `/dev/null`, `/dev/kvm`, `/dev/fuse`,
> and crashing libvirt/QEMU VMs.

### 5.1 `is_rule_globally_dangerous(content) -> bool`
- First **join** backslash-continuations (`\` + newline → space; handles LF/CRLF).
- Then flag any remaining line whose **first key is an assignment** (`=`/`+=`/
  `:=`/`-=`) rather than a match (`==`/`!=`). A line with no leading match key
  matches every device.

### 5.2 `rule_line_has_leading_match_key(line) -> bool`
Skips the key name (`[A-Z_]+`), an optional `{...}` payload (`ATTRS{...}`,
`ENV{...}`, `IMPORT{...}`), and checks the operator is `==`/`!=`.

### 5.3 Repair path
`update_udev_rules` checks `is_rule_globally_dangerous` on the existing rule;
if dangerous, it prints a critical "Repairing globally-dangerous legacy udev
rule" notice and **overwrites** it with the correct single-line form (or purges
it when no fallback rule is needed). Regression tests assert the rendered rule
is always a single safe line starting with a match key, and that the exact
broken form from the bug report is detected.

---

## 6. The systemd User Service

### 6.1 Template (`packaging/linux/systemd/qmkonnect.service.template`)
```ini
[Unit]
Description=QMKonnect - QMK Keyboard Window Notifier
After=graphical-session.target
BindsTo=dev-qmkonnect_device.device      # the symlink the static rule creates
StartLimitBurst=5
StartLimitIntervalSec=60

[Service]
Type=simple
ExecStart=/usr/bin/qmkonnect
Restart=always
RestartSec=5
Environment=RUST_BACKTRACE=1
PrivateTmp=false
ProtectSystem=full
ProtectHome=false
NoNewPrivileges=true
ReadWritePaths=/dev
ReadWritePaths=%t

[Install]
WantedBy=default.target
```
- **`BindsTo=dev-qmkonnect_device.device`**: stops the service when the keyboard
  unplugs; waits for it at boot.
- **`Restart=always`** + `panic="abort"` ⇒ crash recovery without `catch_unwind`.
- The package's `post_install` instantiates it to
  `/usr/lib/systemd/user/qmkonnect.service` and runs
  `systemctl --global enable qmkonnect.service`.

### 6.2 Why the service is optional
The static udev rule's `SYSTEMD_USER_WANTS` starts the service on device arrival
*if it's enabled*. The user can instead run `qmkonnect & disown` directly. The
service is the recommended path for hotplug auto-start.

> **Trayless (`--no-default-features`) build caveat.** The minimal
> `runners/linux.rs` target has no SNI tray, hence **no poll thread**, so it runs
> the capability handshake **once at startup and never again** — an unplug/
> replug after startup is not re-handshaked (host rules will not resume without a
> restart). This is acceptable for the documented trayless-service target
> because `BindsTo=dev-qmkonnect_device.device` stops the unit on unplug and
> `Restart=always` (re)starts it on replug, re-running the startup handshake.
> The full `linux-tray` build does not have this limitation: its poll thread's
> `PresenceTracker` re-handshakes on any capable-board transition.

### 6.3 XDG autostart `.desktop` — the universal fallback (F17)

Alongside the systemd user service, every Linux package ships an XDG autostart
entry at `/etc/xdg/autostart/qmkonnect.desktop` (`PACKAGING.md` §4.7). Every
DE session manager honors `~/.config/autostart/` (and `/etc/xdg/autostart/`), so
this starts the daemon at **login on every desktop — systemd or not** (MX,
Artix, Void, Gentoo). It is the load-bearing autostart path on non-systemd
distros and a belt-and-suspenders on systemd ones.

- **Trade-off vs the service:** the `.desktop` starts at login only; it loses
  the systemd `BindsTo` plug/unplug lifecycle (start on plug, stop on unplug).
  On systemd distros the service remains primary; the `.desktop` is redundant
  but harmless (the single-instance story is owned by the tray/runner, not by
  the launcher).
- **Contents:** `Type=Application`, `Exec=qmkonnect`, `Icon=input-keyboard`,
  `Terminal=false`, `NoDisplay=true` (autostart-only — not in app menus).
- **Disable:** copy to `~/.config/autostart/qmkonnect.desktop` with
  `Hidden=true`, or remove the system file — same convention as every other
  autostart app.
- **Self-install for binary-only installs:** `ensure_xdg_autostart(verbose)`
  (`src/platforms/linux.rs`, called once at startup from `runners/linux.rs`)
  writes the **user** `~/.config/autostart/qmkonnect.desktop` (with
  `Exec=<absolute current_exe()>`) on the first run, gated by the marker file
  `~/.config/qmkonnect/.autostart_initialized` so it never re-enables after the
  user removes it (mirrors the macOS first-run default-on, `UI.md` §6.2). A user
  file shadows a same-named system file in the XDG spec → no double-launch
  alongside a packaged install. This is what makes Scoop / cargo-binstall /
  generic-tarball installs start at login without a package `postinst`.

---

## 7. SNI Tray (`src/linux_tray.rs`, feature `linux-tray`)

StatusNotifierItem over the session D-Bus via **`ksni`** (`features=["blocking"]`),
pure-Rust (no GTK main loop). Runs on **its own D-Bus thread**; the Hyprland
monitor blocks separately on its IPC listener. See `SPEC_UI.md` §1.2–1.4 for the
menu/icon/status details.

### 7.1 `spawn() -> Option<Handle>`
- `QmkTray { device_connected, dark_mode }.assume_sni_available(true).spawn()`:
  - `assume_sni_available(true)` ⇒ register-and-wait rather than hard-failing
    when no SNI host is running. So: no bar at startup ⇒ the item waits silently
    and appears when one starts; no bar at all ⇒ runs headless forever; no
    session D-Bus ⇒ logs the error and runs trayless (returns `None`).
- Poll thread: every **1 s** drive a `PresenceTracker` tick (re-probes
  capable presence via the cache-backed `classify_devices` only when the Tier-1
  path *set* changes — a plug/unplug — so the hot loop never pings on a stable
  bus), every **10** ticks re-query the color-scheme portal; on a transition call
  `handle.update(|t| { t.device_status = …; t.dark_mode = …; })`
  (ksni re-serializes menu + icon; SNI hosts repaint).

### 7.2 Color-scheme detection (`detect_dark_mode`)
Shells out to `dbus-send` reading
`org.freedesktop.portal.Settings.Read org.freedesktop.appearance color-scheme`
(`1`=dark, `2`=light, `0`=no pref → default **dark**). `dbus-send` is chosen over a
zbus variant-deserialization coupling. `parse_color_scheme` is unit-tested.

### 7.3 Why no notify-rust
The "Show Window Information" notification uses `notify-send` (shelled out)
because `notify-rust`'s blocking `show()` spawns a nested tokio runtime, which
panics inside ksni's handler thread.

### 7.4 GNOME: the SNI holdout (AppIndicator)
Stock **GNOME** dropped SNI/AppIndicator support years ago, so the ksni item is
**invisible** on a default GNOME session — the daemon still runs headless
(`spawn()` returns `None` gracefully on no SNI host; `UI.md` §1.2). Two honest
options, both already covered by the code, so no GNOME-specific tray is built:

- **Install the *AppIndicator and KStatusNotifierItem Support* GNOME extension**
  (the standard repackaging of the old KStatusNotifier bridge). With it, ksni's
  item renders normally in the top bar. This is the documented GNOME path.
- **Run trayless.** The daemon, device-status, and rules/settings are fully
  functional without a tray icon — only the click menu is unavailable (use the
  CLI flags: `--list-devices`, `--validate-rules`, etc.).

Do **not** build a GNOME-native tray. The window-detection story on GNOME is the
Shell extension (`PLATFORMS.md` §8), which is independent of the tray; the two
are solved separately.

---

## 8. Config Paths (Linux)

`get_config_paths()` (`src/platforms/linux.rs`), in order:
1. `$XDG_CONFIG_HOME/qmkonnect/config.toml`
2. `~/.config/qmkonnect/config.toml`
3. `/etc/qmkonnect/config.toml`

`create_config_dir()` → `$XDG_CONFIG_HOME/qmkonnect` or
`~/.config/qmkonnect`.

> All platforms now use `QMKonnect/` (Linux: `qmkonnect/` per XDG convention) —
> unified ahead of the first beta.

---

## 9. Linux Dependencies (`Cargo.toml`)

```toml
[target.'cfg(target_os = "linux")'.dependencies]
hyprland   = { version = "0.4.0-beta.2", optional = true }   # feature "hyprland"
libxdo     = "0.6"
tempfile   = "3.0"
libc       = "0.2"                          # only geteuid() for root-aware reload
ksni       = { version = "0.3", optional = true, features = ["blocking"] }   # feature "linux-tray"
gtk        = { version = "0.18", optional = true }                           # feature "linux-tray" (window-info popup)
```

- **`libxdo`** is an unconditional dep (used by the non-default X11 path).
- **`gtk` 0.18** is already compiled into the binary via libappindicator/
  tray-icon, so the GTK popup reuses it (free dep). Runs on a dedicated thread;
  ksni's IPC thread stays pure-IPC.
- **Platform libs:** Ubuntu/Debian `libxdo-dev libudev-dev`; Fedora
  `libxdo-devel systemd-devel`. The Arch build links `-lhidapi-hidraw` (not
  `-lhidapi-libusb`) so usage/usage_page matching works.

---

*Continue with `SPEC_CONFIG.md`.*
