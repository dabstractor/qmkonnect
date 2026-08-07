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