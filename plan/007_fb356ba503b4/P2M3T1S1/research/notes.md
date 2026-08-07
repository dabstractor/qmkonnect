# Research Notes — P2.M3.T1.S1 (GNOME Shell Extension artifact)

> Scope: ONLY the GNOME Shell extension package at `packaging/gnome-shell-extension/`
> (`metadata.json`, `extension.js`, optional `prefs.js`/`stylesheet.css`, the
> D-Bus introspection XML, and a README). This is a **GJS/JavaScript** artifact —
> the only non-Rust deliverable in the plan — so there are NO in-repo JS patterns
> to mirror. Context came from 3 `researcher` subagents (stable GIO/GJS API
> knowledge; they lacked web tools) + direct web verification of the 3
> load-bearing facts (GNOME version, `version`-field type, focus signal).

## 1. Scope boundary (what is IN vs OUT of THIS task)

IN (this task creates, in `packaging/gnome-shell-extension/`):
- `metadata.json` (required) — uuid, name, description, shell-version, version, url.
- `extension.js` (required) — enable/disable/_onFocus + D-Bus export.
- `dbus-interfaces.xml` — D-Bus introspection XML "under the package for reference" (PACKAGING.md §7).
- `README.md` — points users to EGO + the Release asset (DOCS deliverable, §item.5).
- optional `stylesheet.css` (empty/no-op; conventional, harmless).

OUT (owned by OTHER tasks — do NOT create/modify):
- `src/platforms/gnome.rs` (Rust zbus client) → **P2.M3.T2.S1**.
- First-run GNOME notification → **P2.M3.T2.S2**.
- `gnome_probe` body + `construct_backend` "gnome" arm in `src/platforms/linux.rs` → the **client** task. TODAY `gnome_probe` is a STUB returning `Err("GNOME Shell-extension backend not yet implemented (P2.M3)")` (linux.rs:166-169, `#[cfg(feature="gnome")]`). That stub is the CLIENT's to replace; this task must NOT touch it. (The extension PRODUCES the D-Bus name; the client CONSUMES it. Owning the name ⇔ "installed & enabled" is what the client's `gnome_probe` will key on — PLATFORMS.md §6 row 2.)
- `Cargo.toml` `gnome = ["dep:zbus"]` feature → **P2.M1.T2.S2** (ALREADY declared; verified Cargo.toml §2).
- CI zip job (`.github/workflows/release.yml` GNOME extension job) → **P2.M7.T2.S1**.
- `.gitignore` → NEVER modify (forbidden). The zip is a transient CI build output (attached to the Release, never committed); the SOURCE `.js`/`.json` are tracked normally. No gitignore entry is needed or allowed.

## 2. The D-Bus contract (authoritative — PLATFORMS.md §8.1)

- Well-known bus name: **`io.mulletware.QMKonnect`** (owned ⇔ extension installed & enabled).
- Object path: **`/io/mulletware.QMKonnect`**.
- Interface: **`io.mulletware.QMKonnect.WindowMonitor`**.
- method **`GetActiveWindow() → (s app_class, s title)`** — synchronous current-state read.
- signal **`ActiveWindowChanged(s app_class, s title)`** — on focus transition + on `enable()` (initial).
- read properties **`AppClass:s`**, **`Title:s`** — for `org.freedesktop.DBus.Properties` polling.
- `app_class` = **`MetaWindow.get_wm_class()`** (WM_CLASS *class*; parity with the X11 backend — e.g. `Firefox`, `Gnome-terminal`). `title` = **`MetaWindow.get_title()`**.

## 3. Canonical GJS D-Bus EXPORT pattern (high-confidence stable GIO ABI)

Use **`Gio.bus_own_name` + `connection.register_object_with_closures`** (NOT the vtable
variant; NOT `Gio.DBusExportedObject.wrapJSObject` which is GJS-only magic):

- `Gio.bus_own_name(Gio.BusType.SESSION, NAME, Gio.BusNameOwnerFlags.NONE, busAcquiredCb, null, null)` — register the object **inside the 4th arg (bus-acquired)**, NOT name-acquired, to avoid a "name owned / object missing" race.
- Parse XML → InterfaceInfo: `Gio.DBusNodeInfo.new_for_xml(XML).lookup_interface(IFACE)`. XML **MUST have `<node>` root** (bare `<interface>` throws — #1 first-attempt bug).
- `connection.register_object_with_closures(path, ifaceInfo, methodCallCb, getPropertyCb, setPropertyCb)` → returns a guint registration id (0 = failure). Pass **arrow functions** so `this` binds to the extension instance.
- Save 3 handles: `ownerId` (from bus_own_name), `registrationId` (from register_object_with_closures), `connection`.
- Reply: `invocation.return_value(GLib.Variant('(ss)', [appClass, title]))` — the **full out-tuple `(ss)`**, never two separate variants.
- Signal: `connection.emit_signal(null, path, iface, name, GLib.Variant('(ss)', [a, b]))` — `null` dest = broadcast.
- Property get: `GLib.Variant('s', val)`. (The get-property closure MUST exist even for read-only props, else `Properties.Get` errors.)
- Unknown method: `invocation.return_dbus_error('org.gtk.GDBus.Error.UnknownMethod', msg)`.
- `disable()` order: `connection.unregister_object(registrationId)` FIRST → `Gio.bus_unown_name(ownerId)` → null refs (Shell review flags leaked objects otherwise).

API docs (docs.gtk.org deterministic scheme): bus_own_name, bus_unown_name,
DBusConnection.register_object_with_closures / unregister_object / emit_signal,
DBusMethodInvocation.return_value / return_dbus_error, DBusNodeInfo.new_for_xml /
lookup_interface, BusNameOwnerFlags. Reference impls: gnome-shell `js/ui/shellDBus.js`
(https://gitlab.gnome.org/GNOME/gnome-shell/-/blob/main/js/ui/shellDBus.js), the
ddterm extension (https://github.com/ddterm/gnome-shell-extension-ddterm), gjs
examples (https://gitlab.gnome.org/GNOME/gjs/-/tree/master/examples).

## 4. Focus tracking + lifecycle (PLATFORMS.md §8.2; Mutter source)

- Signal: **`global.display.connect('notify::focus-window', cb)`** — the GObject `notify`
  for `Meta.Display`'s `:focus-window` property. CONFIRMED: there is NO
  `window-focused`/`window-managed` signal on `Meta.Display`; `notify::focus-window` is the
  single correct hook. (mutter src/core/display.c.)
- Read current window: `global.display.focus_window` → `Meta.Window | null`.
- `window.get_wm_class()` and `window.get_title()` MAY be `null` → fall back to `""`.
- **Call `_onFocus()` once manually in `enable()`** — the signal only fires on CHANGE, so
  without the manual call you'd never emit the window focused at enable time (initial state).
- `enable()`/`disable()` must be **idempotent + reversible** (can fire >1×: lock-screen
  transitions). Everything allocated in enable() is torn down in disable(). **Top-level ESM
  imports only** — no `import` inside functions (ESM static hoisting).
- Dedup: compare last-emitted `[appClass, title]`; skip emit if unchanged (focus churns
  within one app). Mirrors the Rust debouncer's compare-then-emit.

> The references subagent suggested preferring `Shell.WindowTracker`→`app.get_id()`
> (`.desktop` id) over raw `get_wm_class()`. **REJECT for this task** — the D-Bus contract
> (§8.1) explicitly pins `app_class = MetaWindow.get_wm_class()` for cross-platform parity
> with the X11 backend / firmware-pattern world. Use `get_wm_class()` exactly as specced;
> the `?? ""` null-coalesce handles the null case. (Noting the alternative here so the
> implementer doesn't "improve" it and break contract parity.)

## 5. ESM imports (GNOME 45+)

```js
import Gio  from 'gi://Gio';
import GLib from 'gi://GLib';
import {Extension} from 'resource:///org/gnome/Shell/Extensions/js/extensions/extension.js';
```
`global` is injected by the shell loader (do NOT import it). GNOME 45 (Sept 2023) was the
hard ESM cutover — `imports.gi.Gio` is gone. `import Meta from 'gi://Meta'` optional (only
for type clarity; methods don't require it).

## 6. metadata.json — field decisions (PLATFORMS.md §8.2)

- `uuid`: `"qmkonnect@mulletware"` (specced).
- `name`: `"QMKonnect"`.
- `description`: one-liner (focus window → QMKonnect layer switching).
- `shell-version`: **`["45","46","47","48","49","50"]`** (STRINGS). VERIFIED:
  system date = **Aug 7 2026**. GNOME release calendar (release.gnome.org/calendar):
  47=2024-09-18, 48=2025-03-20, 49=2025-09-17, **50=2026-03-18** (current stable),
  51 not until ~2026-09. So 45–50 is the exact correct range. GNOME 45 = ESM floor.
  Do NOT add "51" (unreleased). `shell-version` entries are always STRINGS.
- `version`: `"0.2.8"` (string). The spec (§8.2) says **"version = the QMKonnect release"**;
  current Cargo.toml `[package].version = "0.2.8"`. GNOME Shell loads string versions fine
  (the loader stores it as-is; only legacy 3.x did parseInt). EGO's per-upload integer-
  version convention applies to the **manual EGO upload step** (§8.5: "EGO upload is a
  manual maintainer step") — OUT of this code task's automation scope. Follow the spec →
  string `"0.2.8"`. (If a future release wants EGO-friendly integer versioning, the
  maintainer bumps it at upload time; not a gate here.)
- `url`: `"https://github.com/mulletware/qmkonnect"`.
- `settings-schema`: OMIT (no GSettings / no prefs UI in v1).
- `session-modes`: omit (defaults to `["user"]` — correct for a user-session focus tracker).

## 7. Distribution / zip (references brief; PACKAGING.md §7, §8.5)

- EGO zip = **files at archive ROOT** (`metadata.json` + `extension.js` top-level, NOT
  nested under a subdir). Filename `qmkonnect@mulletware.shell-extension.zip`.
- Build: `gnome-extensions pack` (validates as it packs; absent on this Hyprland box) OR a
  flat `zip` one-liner **listing files directly** (never `zip -r out.zip .` or a dir arg —
  both nest under a subdir and EGO rejects it).
- Local test loop: `gnome-extensions pack` → `gnome-extensions install <zip>` →
  `gnome-extensions enable qmkonnect@mulletware` → reload (Alt+F2 `r` on X11; logout/login
  on Wayland). Verify: `gdbus introspect/call/monitor --session --dest io.mulletware.QMKonnect`.
- CI zip job is **P2.M7.T2.S1** (OUT of scope); this task ships the SOURCE files the CI zips.

## 8. Validation strategy (this box = Hyprland, NOT GNOME)

Tooling present: `jq`, `python3`, `node`, `gjs`, `zip`, `unzip`, `gdbus`.
Absent: `gnome-extensions`. Session: Hyprland (no gnome-shell to load the extension).

Therefore the HARD automated gates (run here):
1. **JSON validity + required fields**: `jq -e '.uuid,.name,.description,.shell-version,.version' metadata.json` (non-zero if any null/missing); `python3 -m json.tool metadata.json >/dev/null`.
2. **JS/ESM syntax**: copy → `.mjs` and `node --check` (node parses ESM; unresolved `global`/`Extension` are RUNTIME refs, not syntax errors — `--check` is syntax-only, so it validates the ESM cleanly). `gjs -c` is NOT usable for this (the `resource:///org/gnome/shell/...` import fails standalone; gjs has no gnome-shell resources).
3. **Zip top-level structure**: build the zip with the flat one-liner; `unzip -l` must show `metadata.json`/`extension.js` at the TOP level (no `<dir>/` prefix).

DEFERRED (manual, requires a real GNOME session — not a hard gate on this Hyprland box, but
documented for the implementer): load the zip in GNOME Shell and run `gdbus introspect
--session --dest io.mulletware.QMKonnect --object-path /io/mulletware/QMKonnect`,
`gdbus call --session ... --method ...GetActiveWindow`, and `gdbus monitor --session
--dest io.mulletware.QMKonnect` while switching focus.

## 9. Residual risks (documented, not blocking)

- R1 (low): subagents lacked web tools; all GNOME/GJS technical content is from stable
  GIO C ABI + Mutter/gnome-shell source knowledge. The 3 load-bearing facts (GNOME
  version, `version` type, focus signal) were independently web-verified.
- R2 (medium): the extension can only be RUNTIME-validated inside a real GNOME Shell
  (Hyprland dev box can't load it). The pure-syntax + JSON + zip gates are the automated
  ceiling here; live behavior is a documented manual step.
- R3 (low): `version` as string "0.2.8" is spec-compliant and loads fine; if EGO ever
  enforces integer versioning strictly, the maintainer adjusts at the (manual) upload step.