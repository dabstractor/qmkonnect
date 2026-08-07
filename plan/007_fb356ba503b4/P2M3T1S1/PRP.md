# PRP — P2.M3.T1.S1: GNOME Shell Extension Artifact (`packaging/gnome-shell-extension/`)

> **Repo under change:** QMKonnect (`/home/dustin/projects/qmkonnect`).
> **Files CREATED (all under `packaging/gnome-shell-extension/`):** `metadata.json`,
> `extension.js`, `dbus-interfaces.xml`, `README.md`, `stylesheet.css`.
> **Files NOT touched (owned by OTHER tasks — see §Scope Boundary):** `src/platforms/gnome.rs`
> (P2.M3.T2.S1), the `gnome_probe` stub + `construct_backend` arm in `src/platforms/linux.rs`
> (the *client* task), `Cargo.toml` (P2.M1.T2.S2 — the `gnome` feature is already declared),
> `.github/workflows/release.yml` (the CI zip job is P2.M7.T2.S1), `.gitignore` (never modify).
>
> **⚠️ THIS IS THE ONLY NON-RUST DELIVERABLE IN THE PLAN.** It is a **GJS/JavaScript** GNOME
> Shell extension — there are **no in-repo JS patterns to mirror**. The full reference
> implementation is therefore given verbatim below (§Implementation Blueprint). Do NOT look for
> "an existing extension to copy" in the repo; there isn't one.
>
> **What it does:** GNOME (Mutter) cannot report the active window to client processes, so a
> tiny extension runs *inside* `gnome-shell`'s GJS runtime, reads
> `global.display.focus_window`, and republishes `(WM_CLASS-class, title)` over the session
> D-Bus (`io.mutterware.QMKonnect` → `io.mulletware.QMKonnect.WindowMonitor`), where the
> desktop app's GNOME backend (`src/platforms/gnome.rs`, P2.M3.T2.S1) subscribes. Owning the
> well-known name is the "installed & enabled" presence signal the client probes for
> (PLATFORMS.md §6 row 2). The app binary **cannot** load this extension; it is a SEPARATE
> artifact distributed via extensions.gnome.org + the GitHub Release (§8.5).

---

## Goal

**Feature Goal**: Create the GNOME Shell extension package `qmkonnect@mulletware` under
`packaging/gnome-shell-extension/` — `metadata.json` (uuid/shell-version/version) + `extension.js`
(`enable`/`disable`/`_onFocus`) that acquires the D-Bus well-known name
`io.mulletware.QMKonnect`, exports object `/io/mulletware.QMKonnect` with interface
`io.mulletware.QMKonnect.WindowMonitor` (method `GetActiveWindow()→(ss)`, signal
`ActiveWindowChanged(ss)`, read properties `AppClass`/`Title`), tracks focus via
`global.display.connect('notify::focus-window', …)`, dedups, and emits changes — plus the
D-Bus introspection XML (reference) and a user-facing README. GNOME Shell 45+ ESM APIs.

**Deliverable** (concrete; validates on the dev box TODAY — a Hyprland box, so validation is
JSON/syntax/zip, with live GNOME `gdbus` documented as a manual step):
- `packaging/gnome-shell-extension/metadata.json` — uuid=`qmkonnect@mulletware`, name,
  description, `shell-version: ["45","46","47","48","49","50"]` (strings), version=`"0.2.8"`,
  url=`https://github.com/dabstractor/qmkonnect`.
- `packaging/gnome-shell-extension/extension.js` — `export default class QMKonnectExtension
  extends Extension` with `enable()`/`disable()`/`_onFocus()` + the D-Bus export via
  `Gio.bus_own_name` + `connection.register_object_with_closures` (ESM imports, GNOME 45+).
- `packaging/gnome-shell-extension/dbus-interfaces.xml` — the introspection XML for the
  interface (kept "under the package for reference", PACKAGING.md §7).
- `packaging/gnome-shell-extension/README.md` — install from extensions.gnome.org + from the
  Release asset + from source; compatibility; troubleshooting pointer.
- `packaging/gnome-shell-extension/stylesheet.css` — empty placeholder (conventional; harmless).

**Success Definition**:
- `python3 -m json.tool metadata.json >/dev/null` succeeds; `jq -e` confirms every required
  field (`uuid`,`name`,`description`,`shell-version`,`version`) is present and non-null.
- `node --check` on an `.mjs` copy of `extension.js` reports **no syntax errors** (ESM parsed;
  unresolved `global`/`Extension` are runtime refs, not syntax errors).
- The EGO-format zip builds with **`metadata.json` + `extension.js` at the archive ROOT** (no
  subdirectory nesting) — verified by `unzip -l`.
- `git diff --stat` shows ONLY files under `packaging/gnome-shell-extension/` (NO `src/`,
  `Cargo.toml`, `release.yml`, `.gitignore`, docs/spec/PRD/tasks.json changes).

## User Persona (if applicable)

**Target User**: GNOME desktop users (Ubuntu/Fedora/Debian defaults + anyone on GNOME/Wayland).
This is the **only** backend that reliably works on GNOME/Wayland (Mutter advertises neither
foreign-toplevel protocol; AT-SPI is best-effort).

**Use Case**: A GNOME user installs QMKonnect via their distro package / Flatpak-free tarball.
On first launch the daemon's `gnome_probe` (P2.M3.T2.S1) finds the D-Bus name unowned → fires a
one-shot notification (P2.M3.T2.S2) pointing them at the extension. They install it from
extensions.gnome.org (or the Release `.zip`), enable it, and window-driven layer/keymap
switching starts working.

**User Journey**: install QMKonnect → (GNOME) get the "install the GNOME extension" toast →
install `qmkonnect@mulletware` from EGO / Release zip → enable in the Extensions app → reload
shell → daemon auto-selects the GNOME backend on next probe → focus changes switch the active
layer. No reboot, no reflash.

**Pain Points Addressed**: Without this extension, GNOME/Wayland users get NO window detection
(Mutter exposes no client API). Today `select_linux_backend`'s `gnome_probe` is a stub
(`Err("GNOME Shell-extension backend not yet implemented (P2.M3)")`), so every GNOME user falls
through to AT-SPI (best-effort, off by default) or no-backend. This extension + the client
(P2.M3.T2.S1) make F16 actually work on GNOME.

## Why

- **F16 (PRD §4) = "one binary, every Linux desktop".** GNOME is the single largest Linux
  desktop and the ONLY one where the standard foreign-toplevel/Wayland path is impossible
  (Mutter implements neither protocol). The GNOME Shell extension is the *unique* reliable
  bridge — it runs inside `gnome-shell` (the only process allowed to read `focus_window`) and
  republishes over D-Bus. PLATFORMS.md §8 designates it priority #2 (after foreign-toplevel,
  which GNOME never advertises → GNOME always falls through to here).
- **Owns the name ⇔ installed & enabled.** The client (P2.M3.T2.S1) probes for the well-known
  name's owner as its availability signal (§6 row 2). So acquiring `io.mulletware.QMKonnect`
  in `enable()` and releasing it in `disable()` is load-bearing — it's how the daemon knows the
  extension is present (and drives the §8.4 first-run notification + §8.3 NameOwnerChanged
  re-acquire). This is why we `bus_own_name` rather than just `emit_signal` on an ad-hoc object.
- **Separate deliverable from the app binary.** The app cannot load it (different process,
  different runtime — GJS vs the daemon). It's distributed via extensions.gnome.org + the
  Release asset; CI (P2.M7.T2.S1) zips this directory. So the deliverable here is the SOURCE
  files CI packs, not the zip itself.

## What

A GNOME 45+ ESM extension that:

1. **`enable()`** parses the introspection XML into a `GDBusInterfaceInfo`, owns the name
   `io.mulletware.QMKonnect` on the session bus, and — inside the **bus-acquired** callback —
   exports `/io/mulletware.QMKonnect` with the `WindowMonitor` interface via
   `register_object_with_closures` (method/get-property/set-property closures), then connects
   `global.display.connect('notify::focus-window', …)` and forces an initial-state emit.
2. **`_onFocus()`** reads `global.display.focus_window` → `[w.get_wm_class() ?? '',
   w.get_title() ?? '']` (or `['','']` when null), dedups against the last-emitted pair, updates
   the `AppClass`/`Title` backing state, and emits `ActiveWindowChanged(ss)`.
3. **`disable()`** disconnects the focus signal, `unregister_object`s, `bus_unown_name`s, and
   nulls every reference (idempotent + reversible; the shell re-calls enable/disable on lock-
   screen transitions).
4. **`GetActiveWindow()`** replies synchronously with the current `(ss)` state.
5. Read properties `AppClass`/`Title` are served on demand by the get-property closure
   (required even for read-only props).

### Success Criteria
- [ ] `metadata.json` has uuid=`qmkonnect@mulletware`, `shell-version=["45".."50"]` (strings),
      `version="0.2.8"`, valid JSON, all required fields present.
- [ ] `extension.js` is GNOME 45+ ESM (top-level `import … from 'gi://Gio'` +
      `resource:///org/gnome/Shell/Extensions/js/extensions/extension.js`; `export default class
      … extends Extension`); no `imports.*` legacy syntax; no `import` inside functions.
- [ ] D-Bus contract exact: name `io.mulletware.QMKonnect`, path
      `/io/mulletware.QMKonnect`, interface `io.mulletware.QMKonnect.WindowMonitor`,
      method `GetActiveWindow()→(ss)`, signal `ActiveWindowChanged(ss)`, props `AppClass`/`Title`.
- [ ] `app_class` = `MetaWindow.get_wm_class()` (NOT `Shell.WindowTracker`/`app.get_id()` —
      contract parity with the X11 backend); null-coalesced to `""`.
- [ ] Focus tracked via `global.display.connect('notify::focus-window', …)`; initial state
      emitted on enable (forced past dedup).
- [ ] `enable()`/`disable()` are idempotent + reversible; every handle (`ownerId`,
      `registrationId`, `connection`, `focusId`) is saved in enable and torn down in disable.
- [ ] zip builds with `metadata.json`+`extension.js` at archive ROOT (no nesting).

## All Needed Context

### Context Completeness Check

_Pass._ An agent with no prior knowledge can implement this from this PRP + the repo because:
(a) this is the **only non-Rust deliverable**, so the full reference implementation is given
verbatim (no "find an existing pattern" wild goose chase); (b) the exact D-Bus contract
(name/path/iface/method/signal/props + the `get_wm_class()` parity decision) is pinned to
spec/PLATFORMS.md §8.1 with the rejected alternative (`app.get_id()`) called out so the
implementer doesn't "improve" it; (c) the canonical GJS export API
(`bus_own_name`+`register_object_with_closures`+`new_for_xml`) is given with the two failure
modes to avoid (bare-`<interface>` XML root; register in name-acquired vs bus-acquired); (d) the
scope boundary with the parallel sibling (P2.M3.T2.S1 owns `gnome.rs` + the `gnome_probe`/client
wiring; P2.M7.T2.S1 owns the CI zip; P2.M1.T2.S2 owns Cargo.toml) is explicit, with the exact
`gnome_probe` STUB line cited so it isn't accidentally touched; (e) the validation commands are
pinned to tooling verified present on this Hyprland box (`jq`,`node`,`zip`) and the live-GNOME
`gdbus` gate is documented as manual. Full evidence in `research/notes.md`.

### Documentation & References

```yaml
# MUST READ — the authoritative contract (the WHAT). §8.1 is the D-Bus contract; §8.2 is THIS
# extension's enable/disable/_onFocus spec; §8.5 is the distribution (EGO + Release zip).
- file: spec/PLATFORMS.md
  why: "§8 GNOME Backend (whole section). §8.1: name io.mulletware.QMKonnect, path
        /io/mulletware.QMKonnect, iface io.mulletware.QMKonnect.WindowMonitor, method
        GetActiveWindow()->(ss), signal ActiveWindowChanged(ss), props AppClass/Title,
        app_class=MetaWindow.get_wm_class(). §8.2: enable acquires name + exports object +
        connects notify::focus-window + emits initial; _onFocus reads focus_window ->
        [get_wm_class() ?? '', get_title() ?? ''] or ['',''], dedups, emits; disable
        disconnects + releases name + unexports. §6 row 2: GNOME probe = name is OWNED (why
        owning the name is load-bearing). §8.4/§8.5: first-run notification + EGO/Release
        distribution."
  pattern: "mirror §8.2's pseudocode literally (it is unambiguous)."

# MUST READ — the artifact spec (the WHAT goes WHERE).
- file: spec/PACKAGING.md
  why: "§7 GNOME Shell Extension Artifact: contents = metadata.json (uuid, shell-version,
        version) + extension.js (enable/disable/_onFocus from §8.2) + optional prefs.js +
        stylesheet.css + 'D-Bus interface introspection XML under the package for reference'.
        Build = zip as qmkonnect@mulletware.shell-extension.zip (EGO format); CI builds it
        (§9, owned by P2.M7.T2.S1). §11: the zip is a gitignored build output (never commit)."

# MUST READ — the parallel sibling PRP (the CONSUMER contract; do NOT duplicate its work).
- file: plan/007_fb356ba503b4/P2M3T1S1/research/notes.md
  why: "this task's OWN evidence trail: the canonical GJS export API, the metadata.json field
        decisions (shell-version strings 45-50, version '0.2.8'), the focus-signal
        confirmation, the validation strategy for a Hyprland box, and the scope boundary."

# REFERENCE — the exact `gnome_probe` STUB this extension PRODUCES the name for (READ ONLY —
# do NOT edit; the client task P2.M3.T2.S1 replaces it).
- file: src/platforms/linux.rs
  why: "lines 166-169: #[cfg(feature=\"gnome\")] fn gnome_probe(_verbose) -> Result<(),String>
        { Err(\"GNOME Shell-extension backend not yet implemented (P2.M3)\") } — the STUB. The
        client (P2.M3.T2.S1) replaces its body to probe for name ownership. THIS task must NOT
        touch it. It confirms the candidate name is the STRING 'gnome' (line 50: ('gnome',
        gnome_probe as ProbeFn)) and that the feature gate is feature=\"gnome\" (already in
        Cargo.toml §2 / default)."

# REFERENCE — the canonical GJS export pattern (stable GIO C ABI; GJS binds 1:1).
- url: https://docs.gtk.org/gio/method.DBusConnection.register_object_with_closures.html
  why: "the export entry point: register_object_with_closures(connection, path, ifaceInfo,
        methodCallCb, getPropertyCb, setPropertyCb) -> guint registrationId (0 on failure).
        Takes plain JS functions (use ARROW fns so `this` binds to the extension instance)."
  critical: "use the _with_closures variant, NOT register_object (needs a fiddly vtable boxed
             struct) and NOT Gio.DBusExportedObject.wrapJSObject (GJS-only magic, not in the
             GIO C reference)."
- url: https://docs.gtk.org/gio/func.bus_own_name.html
  why: "Gio.bus_own_name(BusType.SESSION, name, BusNameOwnerFlags.NONE, busAcquiredCb,
        nameAcquiredCb, nameLostCb) -> guint ownerId. REGISTER THE OBJECT INSIDE THE
        bus-acquired callback (4th arg), NOT name-acquired — avoids a name-owned/object-missing
        race."
  critical: "the three callbacks are all optional (pass null); GJS drops user_data/destroy, so
             you pass exactly 6 args."
- url: https://docs.gtk.org/gio/ctor.DBusNodeInfo.new_for_xml.html
  why: "Gio.DBusNodeInfo.new_for_xml(xml) -> DBusNodeInfo; .lookup_interface(name) ->
        GDBusInterfaceInfo. The XML MUST have a <node> ROOT (a bare <interface> root throws —
        #1 first-attempt bug)."
- url: https://docs.gtk.org/gio/method.DBusMethodInvocation.return_value.html
  why: "invocation.return_value(GLib.Variant('(ss)', [appClass, title])) — the Variant MUST be
        the FULL out-tuple '(ss)', never two separate 's' variants (mismatch throws 'Message
        body does not match expected body type')."
- url: https://docs.gtk.org/gio/method.DBusConnection.emit_signal.html
  why: "connection.emit_signal(dest, path, iface, sig, GLib.Variant('(ss)', [a,b])); dest=null
        broadcasts to all subscribers (the daemon's zbus proxy receives it)."
- url: https://docs.gtk.org/gio/method.DBusConnection.unregister_object.html
  why: "connection.unregister_object(registrationId) -> bool. Call this BEFORE bus_unown_name
        in disable() (no name-owned/object-gone window)."

# REFERENCE — focus tracking (Mutter source).
- url: https://gitlab.gnome.org/GNOME/mutter/-/blob/main/src/core/display.c
  why: "confirms `:focus-window` is a GObject property on MetaDisplay -> the notify signal is
        exactly `notify::focus-window`; read current via global.display.focus_window
        (Meta.Window | null). There is NO window-focused/window-managed signal on Meta.Display."

# REFERENCE — GNOME 45+ ESM + lifecycle + metadata schema (the official guide).
- url: https://gjs.guide/extensions/upgrading/gnome-shell-45.html#es-modules
  why: "GNOME 45 = ESM floor: `import X from 'gi://X'` + `import … from 'resource:///…'`; the
        legacy imports.* object is gone. Top-level imports ONLY (no import inside functions)."
- url: https://gjs.guide/extensions/overview/anatomy.html
  why: "enable()/disable() contract (idempotent, reversible; re-called on lock-screen
        transitions) + metadata.json required fields (uuid/name/description/shell-version/
        version) + file layout."

# REFERENCE — production D-Bus export inside gnome-shell (the gold-standard example).
- url: https://gitlab.gnome.org/GNOME/gnome-shell/-/blob/main/js/ui/shellDBus.js
  why: "gnome-shell's OWN exported services (org.gnome.Shell, org.gnome.Shell.Extensions,
        org.gnome.Shell.Introspect). The introspection-XML + own-name pattern at scale."

# REFERENCE — a real 45+ extension exporting D-Bus to an external daemon (our exact pattern).
- url: https://github.com/ddterm/gnome-shell-extension-ddterm
  why: "ddterm exports a D-Bus interface so an external subprocess drives the terminal — the
        same 'extension inside gnome-shell talks to an outside process' architecture as
        QMKonnect."
```

### Current Codebase tree (relevant subset)

```bash
spec/
  PLATFORMS.md          # §8 = authoritative contract for THIS task (§8.1 D-Bus, §8.2 extension)  ← READ
  PACKAGING.md          # §7 = artifact spec; §8.5/§9 = distribution + CI zip (P2.M7.T2.S1)       ← READ
src/platforms/
  linux.rs              # lines 45-60 linux_backend_candidates (candidate name = "gnome"); 166-169 gnome_probe STUB  ← READ ONLY (do NOT edit)
  mod.rs                # WindowMonitor trait (the CONSUMER side; P2.M3.T2.S1 implements it)      ← READ ONLY
Cargo.toml              # §2: gnome = ["dep:zbus"] already in `default` (P2.M1.T2.S2 owns it)     ← DO NOT TOUCH
packaging/
  (NO gnome-shell-extension/ dir exists yet)        ← CREATE this directory + its files
```

### Desired Codebase tree (files this task creates)

```bash
packaging/gnome-shell-extension/        # NEW directory
├── metadata.json                        # NEW — uuid/shell-version/version/name/description/url
├── extension.js                         # NEW — enable/disable/_onFocus + D-Bus export (GNOME 45+ ESM)
├── dbus-interfaces.xml                  # NEW — introspection XML (reference; PACKAGING.md §7)
├── README.md                            # NEW — install (EGO / Release / source) + compat + troubleshooting
└── stylesheet.css                       # NEW — empty placeholder (conventional; harmless)
# (NO src/, Cargo.toml, release.yml, .gitignore, docs/spec/PRD/tasks.json changes)
```

### Known Gotchas of our codebase & Library Quirks

```javascript
// CRITICAL (GOTCHA-1 — this is NOT Rust; no in-repo pattern exists). This is the ONLY
//   JavaScript deliverable in the plan. Do NOT search src/ for a pattern to mirror. The full
//   reference extension.js is in §Implementation Blueprint — implement it verbatim.

// CRITICAL (GOTCHA-2 — the D-Bus contract pins app_class = get_wm_class(), NOT app.get_id()).
//   A subagent suggested preferring Shell.WindowTracker → app.get_id() (the .desktop id). REJECT
//   it: spec/PLATFORMS.md §8.1 explicitly pins app_class = MetaWindow.get_wm_class() for PARITY
//   with the X11 backend / the firmware-pattern world (e.g. Firefox, Gnome-terminal). Using
//   app.get_id() would break cross-platform contract parity. Use get_wm_class() exactly.

// CRITICAL (GOTCHA-3 — register the object in the BUS-ACQUIRED callback, not name-acquired).
//   Gio.bus_own_name's 4th arg (busAcquired) fires when the connection is up but BEFORE the
//   name is published; register_object_with_closures THERE so the object exists when the name
//   goes live. Registering in the 5th arg (nameAcquired) creates a name-owned/object-missing
//   race window. All three callbacks are optional (pass null for the ones you don't use).

// CRITICAL (GOTCHA-4 — the introspection XML MUST have a <node> root). A bare <interface> root
//   throws a parse error in Gio.DBusNodeInfo.new_for_xml (the #1 first-attempt bug). Always
//   wrap: <node><interface name="…">…</interface></node>.

// CRITICAL (GOTCHA-5 — the get-property closure MUST exist even for read-only props). Without
//   it, org.freedesktop.DBus.Properties.Get errors. Return GLib.Variant('s', value); return
//   null for unknown props.

// CRITICAL (GOTCHA-6 — return_value's Variant must be the FULL out-tuple '(ss)'). Never two
//   separate 's' variants; never a single 's'. Mismatch throws
//   "Message body of type … does not match expected body type".

// CRITICAL (GOTCHA-7 — arrow functions for the register_object_with_closures callbacks). The
//   three closures must reach the extension instance state (this._appClass, this._title). Use
//   ARROW functions (() => {...}) so `this` is lexically bound. A `function(){}` loses `this`
//   and silently breaks method/property replies. (The notify::focus-window handler, by
//   contrast, is bound via this._onFocus.bind(this).)

// CRITICAL (GOTCHA-8 — force the initial-state emit past dedup). notify::focus-window fires
//   only on CHANGE, so enable() must emit the current window once. BUT _onFocus() dedups
//   against _lastEmitted — and a focus change between enable() and the async bus-acquired
//   callback can set _lastEmitted early, causing the initial emit to be deduped away. FIX: in
//   the bus-acquired callback, reset this._lastEmitted = null; BEFORE calling this._onFocus()
//   so the initial emit always goes through.

// GOTCHA-9 (save FOUR handles in enable; tear them all down in disable). ownerId (bus_own_name),
//   registrationId (register_object_with_closures), connection, focusId (the notify signal).
//   disable() order: disconnect focus FIRST (stop churn) -> unregister_object(registrationId)
//   -> bus_unown_name(ownerId) -> null ALL refs (the shell's extension reviewer flags leaked
//   objects). enable/disable are idempotent (guard each teardown with if (...)).

// GOTCHA-10 (GNOME 45+ = ESM only; top-level imports only). import Gio from 'gi://Gio'; (NOT
//   imports.gi.Gio). import {Extension} from 'resource:///org/gnome/Shell/Extensions/js/
//   extension.js'; (note the Shell-capitalized path segment under ESM). `global` is injected
//   by the shell loader — do NOT import it. NEVER put an import inside a function (ESM static
//   hoisting; runtime error).

// GOTCHA-11 (shell-version entries are STRINGS; the array is the task's exact 45-50 range).
//   "shell-version": ["45","46","47","48","49","50"] — strings, not ints. Verified: system
//   date Aug 2026; GNOME 50 (2026-03-18) is current stable, 51 not until ~Sept 2026. GNOME 45
//   is the ESM floor. Do NOT add "51" (unreleased). Swapping shell-version to ints / version
//   to a non-string is the classic metadata.json bug — but note `version` here IS a string
//   ("0.2.8"), which is correct (GOTCHA-12).

// GOTCHA-12 (version = "0.2.8", a string — spec compliance). spec/PLATFORMS.md §8.2: "version
//   = the QMKonnect release"; Cargo.toml [package].version = "0.2.8". GNOME Shell loads string
//   versions fine (the loader stores as-is; only legacy 3.x did parseInt). EGO's per-upload
//   integer-version convention applies to the MANUAL EGO upload step (§8.5: "EGO upload is a
//   manual maintainer step") — OUT of this code task's automation scope. Use the string.

// GOTCHA-13 (the zip MUST be flat — files at archive ROOT, never nested). EGO + gnome-shell's
//   loader reject a zip where files sit under <uuid>/extension.js. Build with the files listed
//   DIRECTLY (never `zip -r out.zip .` or `zip -r out.zip dir/` — both nest). See Level 3.

// GOTCHA-14 (this box is Hyprland, NOT GNOME — the live gdbus load is a MANUAL gate). `gjs -c`
//   is NOT usable for syntax validation (the resource:///org/gnome/shell/... import fails
//   standalone — gjs has no gnome-shell resources). Use `node --check` on an .mjs COPY for the
//   syntax gate (node parses ESM; unresolved `global`/`Extension` are runtime refs, not syntax
//   errors). gnome-extensions is absent here too. Live validation = install in a real GNOME
//   Shell + gdbus introspect/call/monitor (documented in Level 3, not a hard gate).

// GOTCHA-15 (disable() nulling + idempotence). enable()/disable() can fire >1× (lock-screen
//   transitions with default session-modes:["user"]). Guard every teardown (if (this._focusId),
//   if (this._ownerId), …) and null every reference at the end of disable().
```

## Implementation Blueprint

### Data models and structure

There are no Rust data models — this is GJS. The "model" is the extension class's instance
state + the D-Bus interface. The introspection XML (below) IS the contract schema; it is parsed
once in `enable()` into a `GDBusInterfaceInfo` and reused.

```xml
<!-- packaging/gnome-shell-extension/dbus-interfaces.xml  (also embedded in extension.js) -->
<node>
  <interface name="io.mulletware.QMKonnect.WindowMonitor">
    <!-- method: synchronous current-state read -->
    <method name="GetActiveWindow">
      <arg type="s" name="app_class" direction="out"/>
      <arg type="s" name="title"     direction="out"/>
    </method>
    <!-- signal: emitted on focus transition + on enable (initial state) -->
    <signal name="ActiveWindowChanged">
      <arg type="s" name="app_class"/>
      <arg type="s" name="title"/>
    </signal>
    <!-- read properties (org.freedesktop.DBus.Properties polling) -->
    <property type="s" name="AppClass" access="read"/>
    <property type="s" name="Title"    access="read"/>
  </interface>
</node>
```

### Reference `metadata.json` (implement VERBATIM)

```json
{
    "uuid": "qmkonnect@mulletware",
    "name": "QMKonnect",
    "description": "Reports the focused window's WM class and title to QMKonnect for layer/keymap switching.",
    "shell-version": ["45", "46", "47", "48", "49", "50"],
    "version": "0.2.8",
    "url": "https://github.com/dabstractor/qmkonnect"
}
```
(Field rationale in `research/notes.md` §6: `shell-version` strings 45-50 = GNOME 45 ESM floor → GNOME 50 current stable (Aug 2026); `version` string = spec "QMKonnect release"; `url` = canonical repo org `dabstractor`.)

### Reference `extension.js` (implement VERBATIM — there is no in-repo pattern to mirror)

```javascript
// extension.js — QMKonnect GNOME Shell extension (GNOME 45+ / ESM).
// Republishes the focused window's (WM_CLASS class, title) over the session D-Bus so the
// QMKonnect daemon (src/platforms/gnome.rs, feature `gnome`) can subscribe.
// Contract: spec/PLATFORMS.md §8.1; artifact: spec/PACKAGING.md §7.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

import {Extension} from 'resource:///org/gnome/Shell/Extensions/js/extensions/extension.js';

const BUS_NAME = 'io.mulletware.QMKonnect';
const OBJECT_PATH = '/io/mulletware.QMKonnect';
const INTERFACE_NAME = 'io.mulletware.QMKonnect.WindowMonitor';
const FOCUS_SIGNAL = 'notify::focus-window';

const WINDOW_MONITOR_XML = `
<node>
  <interface name="io.mulletware.QMKonnect.WindowMonitor">
    <method name="GetActiveWindow">
      <arg type="s" name="app_class" direction="out"/>
      <arg type="s" name="title" direction="out"/>
    </method>
    <signal name="ActiveWindowChanged">
      <arg type="s" name="app_class"/>
      <arg type="s" name="title"/>
    </signal>
    <property type="s" name="AppClass" access="read"/>
    <property type="s" name="Title" access="read"/>
  </interface>
</node>`;

export default class QMKonnectExtension extends Extension {
    enable() {
        // Backing state read by GetActiveWindow() + the AppClass/Title properties.
        this._appClass = '';
        this._title = '';

        // Dedup cell: [appClass, title] | null. Focus churns within one app; skip redundant emits.
        this._lastEmitted = null;

        // D-Bus export handles — all torn down in disable() (GOTCHA-9).
        this._connection = null;
        this._registrationId = 0;
        this._ownerId = 0;
        this._focusId = 0;

        this._ifaceInfo = Gio.DBusNodeInfo
            .new_for_xml(WINDOW_MONITOR_XML)   // <node> root mandatory (GOTCHA-4)
            .lookup_interface(INTERFACE_NAME);

        // Own the well-known name ⇔ "installed & enabled" (the client's gnome_probe keys on
        // this — PLATFORMS.md §6 row 2). Register the object inside the BUS-ACQUIRED callback
        // (4th arg), NOT name-acquired, to avoid a name-owned/object-missing race (GOTCHA-3).
        this._ownerId = Gio.bus_own_name(
            Gio.BusType.SESSION,
            BUS_NAME,
            Gio.BusNameOwnerFlags.NONE,
            (connection, _name) => {                       // BUS ACQUIRED
                this._connection = connection;
                this._registrationId = connection.register_object_with_closures(
                    OBJECT_PATH,
                    this._ifaceInfo,
                    // (1) method call — arrow fn so `this` binds to the instance (GOTCHA-7)
                    (_conn, _sender, _path, _iface, method, _params, invocation) => {
                        if (method === 'GetActiveWindow') {
                            // Full out-tuple '(ss)' (GOTCHA-6).
                            invocation.return_value(
                                GLib.Variant('(ss)', [this._appClass, this._title]));
                        } else {
                            invocation.return_dbus_error(
                                'org.gtk.GDBus.Error.UnknownMethod',
                                `Unknown method ${method}`);
                        }
                    },
                    // (2) get property — REQUIRED even for read-only props (GOTCHA-5)
                    (_conn, _sender, _path, _iface, prop) => {
                        if (prop === 'AppClass') return GLib.Variant('s', this._appClass);
                        if (prop === 'Title')    return GLib.Variant('s', this._title);
                        return null;
                    },
                    // (3) set property — none writable
                    () => false,
                );
                // Emit the current window now that we're on the bus. Reset the dedup cell first
                // so the initial emit isn't skipped (a pre-connection focus change may have set
                // it) — GOTCHA-8.
                this._lastEmitted = null;
                this._onFocus();
            },
            null,   // name acquired (unused)
            null,   // name lost (unused)
        );

        // Focus tracking. notify::focus-window fires only on CHANGE (GOTCHA: the initial-state
        // emit is handled in the bus-acquired callback above). `.bind(this)` keeps `this`.
        this._focusId = global.display.connect(FOCUS_SIGNAL, this._onFocus.bind(this));
    }

    disable() {
        // Disconnect focus FIRST (stop state churn), then tear down D-Bus (GOTCHA-9).
        if (this._focusId) {
            global.display.disconnect(this._focusId);
            this._focusId = 0;
        }
        // Unregister the object BEFORE releasing the name (no name-owned/object-gone window).
        if (this._registrationId && this._connection) {
            this._connection.unregister_object(this._registrationId);
        }
        this._registrationId = 0;
        if (this._ownerId) {
            Gio.bus_unown_name(this._ownerId);
        }
        this._ownerId = 0;
        // Null every reference (shell reviewer flags leaked objects) — GOTCHA-15.
        this._connection = null;
        this._ifaceInfo = null;
        this._appClass = '';
        this._title = '';
        this._lastEmitted = null;
    }

    // Read the focused window -> [app_class, title] (['',''] when nothing focused), dedup
    // against the last-emitted pair, update state, and broadcast ActiveWindowChanged.
    // app_class = MetaWindow.get_wm_class() — contract parity with the X11 backend (GOTCHA-2).
    _onFocus() {
        const window = global.display.focus_window;   // Meta.Window | null
        const appClass = window ? (window.get_wm_class() ?? '') : '';
        const title = window ? (window.get_title() ?? '') : '';

        // Dedup: skip if unchanged since the last emit.
        if (this._lastEmitted !== null &&
            this._lastEmitted[0] === appClass &&
            this._lastEmitted[1] === title) {
            return;
        }
        this._lastEmitted = [appClass, title];

        // Update the backing state read by GetActiveWindow + the properties.
        this._appClass = appClass;
        this._title = title;

        // Broadcast the change (only if the object is already exported).
        if (this._connection) {
            this._connection.emit_signal(
                null,            // destination: null = broadcast to all subscribers
                OBJECT_PATH,
                INTERFACE_NAME,
                'ActiveWindowChanged',
                GLib.Variant('(ss)', [appClass, title]),
            );
        }
    }
}
```

### Reference `stylesheet.css` (implement — empty placeholder)

```css
/* QMKonnect GNOME Shell extension — no styles needed (no UI added to the shell).
   Present so `gnome-extensions pack` includes it and EGO's layout check is happy. */
```

### Reference `README.md` skeleton (implement — see Task 6 for the full text)

Install-from-extensions.gnome.org + from the Release `.zip` + from source; compatibility (GNOME
45-50); troubleshooting (enable, shell reload: Alt+F2 `r` on X11 / logout on Wayland; the app's
first-run notification). Full text in Task 6.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: BASELINE — confirm no pre-existing files / clean tree
  - RUN: ls packaging/gnome-shell-extension/ 2>/dev/null  (expect: NO such directory)
  - RUN: git status --short                              (expect: clean, or only your changes)
  - WHY: this is a brand-new directory; confirms you're not clobbering anything.

Task 2: CREATE packaging/gnome-shell-extension/metadata.json — implement VERBATIM (§Data models)
  - FIELDS: uuid="qmkonnect@mulletware"; name="QMKonnect"; description (one-liner);
    shell-version=["45","46","47","48","49","50"] (STRINGS — GOTCHA-11); version="0.2.8"
    (STRING — GOTCHA-12); url="https://github.com/dabstractor/qmkonnect".
  - DO NOT add settings-schema (no GSettings/prefs UI in v1). DO NOT add session-modes
    (defaults to ["user"], correct).
  - VALIDATE immediately: python3 -m json.tool metadata.json >/dev/null  (zero exit = valid JSON)
    AND jq -e '.uuid,.name,.description,.["shell-version"],.version' metadata.json >/dev/null
    (zero exit = all required fields present + non-null).
    (jq GOTCHA: the hyphenated key MUST be bracket-quoted as .["shell-version"]; bare
    .shell-version parses as subtraction and throws a jq compile error.)

Task 3: CREATE packaging/gnome-shell-extension/dbus-interfaces.xml — implement VERBATIM (§Data
        models). This is the "D-Bus interface introspection XML under the package for reference"
        (PACKAGING.md §7). It MUST be byte-identical to the WINDOW_MONITOR_XML embedded in
        extension.js (the in-file string is what's actually used at runtime; this file is the
        human-readable contract + reference for the client author).

Task 4: CREATE packaging/gnome-shell-extension/extension.js — implement VERBATIM (§Reference
        extension.js). Enforce EVERY gotcha:
  - ESM imports only, top-level (GOTCHA-10): import Gio/GLib from 'gi://…'; Extension from
    'resource:///org/gnome/Shell/Extensions/js/extensions/extension.js'.
  - export default class QMKonnectExtension extends Extension { enable(){} disable(){} _onFocus(){} }.
  - enable(): parse WINDOW_MONITOR_XML -> lookup_interface; bus_own_name(...busAcquired cb that
    register_object_with_closures the 3 closures [ARROW fns — GOTCHA-7] then emits initial state
    [reset _lastEmitted=null first — GOTCHA-8]); connect notify::focus-window with .bind(this).
  - _onFocus(): focus_window -> [get_wm_class() ?? '', get_title() ?? ''] or ['','']; dedup vs
    _lastEmitted; set _appClass/_title; emit_signal('(ss)') if connection exists. (GOTCHA-2:
    get_wm_class NOT app.get_id.)
  - disable(): disconnect focus; unregister_object; bus_unown_name; null ALL refs. Idempotent.
  - VALIDATE immediately: cp extension.js /tmp/qmk_ext.mjs && node --check /tmp/qmk_ext.mjs
    (zero exit = valid ESM syntax — GOTCHA-14; gjs -c is NOT usable here).

Task 5: CREATE packaging/gnome-shell-extension/stylesheet.css — the empty placeholder above
        (conventional; satisfies gnome-extensions pack + EGO layout).

Task 6: CREATE packaging/gnome-shell-extension/README.md — user-facing docs (DOCS deliverable,
        §item.5). Sections:
    ## QMKonnect GNOME Shell extension
    One paragraph: what it does + that the QMKonnect app (the daemon) is installed separately.
    ## Install (recommended): extensions.gnome.org
    - Search "QMKonnect" on https://extensions.gnome.org, toggle on (link the EGO page once
      published; until then point at the Release asset below).
    ## Install from a Release .zip
    - gnome-extensions install qmkonnect@mulletware.shell-extension.zip
    - gnome-extensions enable qmkonnect@mulletware
    - Reload GNOME Shell: X11 = Alt+F2 then 'r'; Wayland = log out and back in.
    ## Install from source
    - git clone https://github.com/dabstractor/qmkonnect.git
    - cd qmkonnect/packaging/gnome-shell-extension && gnome-extensions pack && gnome-extensions
      install qmkonnect@mulletware.shell-extension.zip && gnome-extensions enable
      qmkonnect@mulletware
    ## Compatibility
    - GNOME Shell 45, 46, 47, 48, 49, 50 (matches metadata.json shell-version). GNOME 45 = first
      ES-modules release (hard floor).
    ## Troubleshooting
    - "Extension did not disable" / no events: re-enable via the Extensions app; on Wayland you
      MUST log out/in (no live shell restart).
    - Verify it's running: `gdbus introspect --session --dest io.mulletware.QMKonnect
      --object-path /io/mulletware/QMKonnect`.
    - The QMKonnect daemon must also be installed (separate package) and running; its first-run
      GNOME notification points here.

Task 7: VALIDATE — the automated gates (run on this Hyprland box; see Validation Loop §Level 1-3)
  - RUN: python3 -m json.tool packaging/gnome-shell-extension/metadata.json >/dev/null
  - RUN: jq -e '.uuid,.name,.description,.shell-version,.version' packaging/gnome-shell-extension/metadata.json
  - RUN: cp packaging/gnome-shell-extension/extension.js /tmp/qmk_ext.mjs && node --check /tmp/qmk_ext.mjs
  - RUN: cd packaging/gnome-shell-extension && zip -r ../qmkonnect@mulletware.shell-extension.zip
        metadata.json extension.js stylesheet.css dbus-interfaces.xml README.md && cd ../..
        then: unzip -l packaging/qmkonnect@mulletware.shell-extension.zip   (files at ROOT, no dir prefix)
  - RUN: git diff --stat   (ONLY files under packaging/gnome-shell-extension/; remove the test zip
        before committing — it's a gitignored build output, PACKAGING.md §11 — but DO NOT add a
        .gitignore entry; the forbidden-ops list forbids editing .gitignore).
```

### Implementation Patterns & Key Details

```javascript
// === The load-bearing export registration (GOTCHA-3) ===
// Register the object INSIDE bus_own_name's bus-acquired callback (4th arg), so the object
// exists before the name goes live:
this._ownerId = Gio.bus_own_name(
    Gio.BusType.SESSION, BUS_NAME, Gio.BusNameOwnerFlags.NONE,
    (connection, _name) => {                 // <-- bus ACQUIRED (4th arg)
        this._connection = connection;
        this._registrationId = connection.register_object_with_closures(
            OBJECT_PATH, this._ifaceInfo,
            (/*method arrow*/)   => { ... invocation.return_value(GLib.Variant('(ss)', [...])) ... },
            (/*get-prop arrow*/) => { ... return GLib.Variant('s', ...) ... },   // GOTCHA-5
            (/*set-prop arrow*/) => false,
        );
        this._lastEmitted = null; this._onFocus();   // GOTCHA-8: force initial emit
    },
    null, null);

// === The dedup + emit (compare-then-emit; mirrors the Rust debouncer) ===
_onFocus() {
    const window = global.display.focus_window;            // Meta.Window | null
    const appClass = window ? (window.get_wm_class() ?? '') : '';   // GOTCHA-2: wm_class, not app_id
    const title    = window ? (window.get_title()    ?? '') : '';
    if (this._lastEmitted && this._lastEmitted[0] === appClass && this._lastEmitted[1] === title) return;
    this._lastEmitted = [appClass, title];
    this._appClass = appClass; this._title = title;
    if (this._connection) {
        this._connection.emit_signal(null, OBJECT_PATH, INTERFACE_NAME, 'ActiveWindowChanged',
            GLib.Variant('(ss)', [appClass, title]));   // GOTCHA-6: full '(ss)' tuple
    }
}
```

### Integration Points

```yaml
D-BUS SESSION BUS (produced by THIS extension, consumed by P2.M3.T2.S1's gnome.rs):
  - name: io.mulletware.QMKonnect (owned ⇔ installed & enabled — the client's gnome_probe keys here)
  - path: /io/mulletware.QMKonnect
  - iface: io.mulletware.QMKonnect.WindowMonitor
  - method GetActiveWindow()->(ss) | signal ActiveWindowChanged(ss) | props AppClass/Title (read)

SELECT_LINUX_BACKEND (src/platforms/linux.rs — NOT modified by this task):
  - candidate name "gnome" (linux.rs:50), feature-gated feature="gnome" (in Cargo.toml default).
  - gnome_probe STUB (linux.rs:166-169) returns Err("…not yet implemented (P2.M3)") TODAY.
    The CLIENT (P2.M3.T2.S1) replaces it to probe for name ownership. This extension is what
    MAKES that probe return Ok once installed+enabled.

CI RELEASE (.github/workflows/release.yml — NOT modified by this task):
  - P2.M7.T2.S1 adds the GNOME-extension job: zip THIS directory ->
    qmkonnect@mulletware.shell-extension.zip, attach to the Release. This task ships the SOURCE
    the CI zips; the zip itself is a gitignored build output (never committed).

CONFIG (CONFIG.md §1.3 — NOT modified by this task):
  - [linux] gnome_poll_interval_ms (default 1000) is the CLIENT's drift-poll cadence. The
    extension just emits GetActiveWindow + the signal; the client polls it. No config here.

DATABASE/ROUTES/CARGO.TOML: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
# JSON validity (metadata.json):
python3 -m json.tool packaging/gnome-shell-extension/metadata.json >/dev/null   # exit 0 = valid
jq -e '.uuid,.name,.description,.["shell-version"],.version' \
   packaging/gnome-shell-extension/metadata.json >/dev/null                     # exit 0 = all required fields present
# jq GOTCHA: bracket-quote the hyphenated key .["shell-version"] (bare .shell-version = subtraction -> compile error)
# ESM syntax (extension.js) — copy to .mjs so node parses it as a module (GOTCHA-14):
cp packaging/gnome-shell-extension/extension.js /tmp/qmk_ext.mjs
node --check /tmp/qmk_ext.mjs                                                    # exit 0 = valid ESM syntax
# Expected: all exit 0. If node reports a SyntaxError, READ the line — ESM imports must be
# top-level (GOTCHA-10); class must be `export default class … extends Extension`.
```

### Level 2: Contract & Content Checks (Component Validation)

```bash
cd /home/dustin/projects/qmkonnect
# Contract parity (the EXACT strings the client keys on):
grep -n 'io.mulletware.QMKonnect' packaging/gnome-shell-extension/extension.js   # name + path(iface) present
grep -n 'GetActiveWindow\|ActiveWindowChanged\|AppClass\|Title' \
     packaging/gnome-shell-extension/extension.js                                 # method/signal/props present
grep -n 'get_wm_class' packaging/gnome-shell-extension/extension.js               # app_class = wm_class (GOTCHA-2)
grep -n "notify::focus-window" packaging/gnome-shell-extension/extension.js       # focus signal (GOTCHA)
grep -nE 'import .* from .gi://|import .* from .resource://|exports.gi' \
     packaging/gnome-shell-extension/extension.js                                 # ESM only; NO imports.* (GOTCHA-10)
# The last grep MUST show only gi:// and resource:// imports; if it matches `imports.`, fix it.
# metadata.json field types:
jq -r '.shell-version | type' packaging/gnome-shell-extension/metadata.json       # -> "array" of strings
jq -r '.version | type'    packaging/gnome-shell-extension/metadata.json         # -> "string" (GOTCHA-12)
jq -r '.shell-version[]'   packaging/gnome-shell-extension/metadata.json         # 45..50 as STRINGS
```

### Level 3: EGO-Format Zip Build (System Validation)

```bash
cd /home/dustin/projects/qmkonnect/packaging/gnome-shell-extension
# Build the EGO-format zip — files listed DIRECTLY (GOTCHA-13: NEVER `zip -r out .` or a dir arg):
zip -r ../qmkonnect@mulletware.shell-extension.zip \
    metadata.json extension.js stylesheet.css dbus-interfaces.xml README.md
cd ../..
# CRITICAL: files MUST be at the archive ROOT (no <dir>/ prefix — EGO + gnome-shell reject nesting):
unzip -l packaging/qmkonnect@mulletware.shell-extension.zip
# Expected: every entry has NO leading directory (e.g. "metadata.json", NOT "gnome-shell-extension/metadata.json").
# CLEAN UP the test zip (it's a gitignored build output; never commit it):
rm -f packaging/qmkonnect@mulletware.shell-extension.zip
```

### Level 4: Creative & Domain-Specific Validation (GNOME session — MANUAL, deferred)

> This dev box is **Hyprland** (not GNOME), so the extension CANNOT be loaded here. The live
> load + `gdbus` smoke is a MANUAL step documented for the implementer / a GNOME VM, NOT a hard
> gate. The automated ceiling (Levels 1-3) covers JSON validity, ESM syntax, contract parity,
> and zip layout — everything verifiable without a running gnome-shell.

```bash
# In a real GNOME 45-50 session (or a GNOME VM):
# 1. Build + install + enable:
cd packaging/gnome-shell-extension
gnome-extensions pack
gnome-extensions install qmkonnect@mulletware.shell-extension.zip
gnome-extensions enable qmkonnect@mulletware
# 2. Reload: X11 = Alt+F2 then type 'r' Enter; Wayland = log out and back in.
# 3. Verify the name is owned + the object is exported:
gdbus introspect --session --dest io.mulletware.QMKonnect --object-path /io/mulletware.QMKonnect
#    Expected: shows interface io.mulletware.QMKonnect.WindowMonitor with GetActiveWindow,
#    ActiveWindowChanged, AppClass, Title.
# 4. Call the method (switch to a window first):
gdbus call --session --dest io.mulletware.QMKonnect --object-path /io/mulletware/QMKonnect \
  --method io.mulletware.QMKonnect.WindowMonitor.GetActiveWindow
#    Expected: ('Firefox', 'some page — Mozilla Firefox')  i.e. a (ss) tuple.
# 5. Watch the signal live while switching focus:
gdbus monitor --session --dest io.mulletware.QMKonnect
#    Expected: an ActiveWindowChanged (ss) on each genuine focus transition (deduped within one app).
# 6. Disable via the Extensions app -> the name should be RELEASED (gdbus introspect fails with
#    the name not owned); re-enable -> name re-acquired. (Confirms disable()/enable() cleanup.)
```

## Final Validation Checklist

### Technical Validation
- [ ] `python3 -m json.tool metadata.json` succeeds (valid JSON).
- [ ] `jq -e` confirms uuid/name/description/shell-version/version all present + non-null.
- [ ] `node --check` on the `.mjs` copy of `extension.js` reports no syntax errors.
- [ ] The EGO zip builds with `metadata.json`+`extension.js` at the archive ROOT (`unzip -l`).
- [ ] `git diff --stat` shows ONLY files under `packaging/gnome-shell-extension/` (NO `src/`,
      `Cargo.toml`, `release.yml`, `.gitignore`, docs/spec/PRD/tasks.json changes).
- [ ] The test zip is removed (not committed); `.gitignore` was NOT modified.

### Feature Validation (parity with PLATFORMS.md §8.1 / §8.2)
- [ ] metadata.json: uuid=`qmkonnect@mulletware`; shell-version=`["45".."50"]` (strings); version=`"0.2.8"`.
- [ ] extension.js is GNOME 45+ ESM (`import … from 'gi://…'` + `resource:///…`; `export default
      class … extends Extension`); NO `imports.*`; NO `import` inside functions.
- [ ] D-Bus contract exact: name `io.mulletware.QMKonnect`, path `/io/mulletware.QMKonnect`,
      iface `io.mulletware.QMKonnect.WindowMonitor`, method `GetActiveWindow()→(ss)`, signal
      `ActiveWindowChanged(ss)`, props `AppClass`/`Title` (read).
- [ ] `app_class` = `MetaWindow.get_wm_class()` (NOT `app.get_id()` — GOTCHA-2), null-coalesced to `""`.
- [ ] Focus via `global.display.connect('notify::focus-window', …)`; initial state emitted on
      enable (forced past dedup — GOTCHA-8).
- [ ] `enable()` owns the name in the **bus-acquired** callback (GOTCHA-3); exports via
      `register_object_with_closures` with arrow-fn closures (GOTCHA-7); get-property closure
      present (GOTCHA-5); `return_value`/`emit_signal` use the full `(ss)` tuple (GOTCHA-6).
- [ ] `disable()` is idempotent + reversible: disconnects focus, `unregister_object`,
      `bus_unown_name`, nulls every reference (GOTCHA-9/15).

### Code Quality Validation
- [ ] The full reference `extension.js` is implemented verbatim (no in-repo pattern to mirror).
- [ ] `dbus-interfaces.xml` is byte-identical to the `WINDOW_MONITOR_XML` in extension.js.
- [ ] README.md points users to extensions.gnome.org + the Release asset + from-source.
- [ ] stylesheet.css is the empty placeholder (conventional).
- [ ] Scope respected: NO `gnome.rs`, NO `gnome_probe`/`construct_backend` edit (P2.M3.T2.S1),
      NO Cargo.toml (P2.M1.T2.S2), NO release.yml (P2.M7.T2.S1), NO `.gitignore`.

### Documentation & Deployment
- [ ] Mode A: spec/PLATFORMS.md §8 is the reference (no docs/* prose added by this task).
- [ ] The extension's own README.md (in-package) is the user-facing doc.
- [ ] The §8.4 first-run notification (pointing here) is owned by P2.M3.T2.S2 — NOT this task.

---

## Anti-Patterns to Avoid

- ❌ Do NOT use `app.get_id()` / `Shell.WindowTracker` for `app_class` — the contract pins
  `get_wm_class()` for X11-backend parity (GOTCHA-2).
- ❌ Do NOT register the object in `bus_own_name`'s name-acquired callback — use bus-acquired
  (GOTCHA-3) to avoid the name-owned/object-missing race.
- ❌ Do NOT write a bare-`<interface>` introspection XML — wrap in `<node>` (GOTCHA-4).
- ❌ Do NOT omit the get-property closure (read props still need it — GOTCHA-5).
- ❌ Do NOT reply with a single `'s'` variant — method returns the full `(ss)` tuple (GOTCHA-6).
- ❌ Do NOT use `function(){}` for the export closures — arrow fns bind `this` (GOTCHA-7).
- ❌ Do NOT forget the forced initial emit (`_lastEmitted = null` before the first `_onFocus()`
  in the bus-acquired callback — GOTCHA-8).
- ❌ Do NOT use `imports.*` or put an `import` inside a function — GNOME 45+ ESM only,
  top-level (GOTCHA-10).
- ❌ Do NOT make shell-version ints or add "51" (unreleased) — strings, 45-50 (GOTCHA-11).
- ❌ Do NOT build the zip with `zip -r out .` or a directory arg — nest and EGO rejects it
  (GOTCHA-13).
- ❌ Do NOT touch `src/platforms/gnome.rs`, `gnome_probe`, `Cargo.toml`, `release.yml`, or
  `.gitignore` — each is owned by a sibling task (§Scope Boundary).