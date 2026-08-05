# Research Notes — P1.M1.T1.S1: list_foreground_windows() → initial_class

> Two-token bug fix in `src/platforms/hyprland.rs`. Makes the "Show Window
> Information" dialog report the same identifier the notify path sends to firmware.

## 1. Verified current state (`src/platforms/hyprland.rs`)

`list_foreground_windows()` @ L559, returns `Vec<(String, String)>` (class, title):

```rust
pub fn list_foreground_windows() -> Vec<(String, String)> {
    let clients = match Clients::get() { ... };
    let mut rows: Vec<(String, String)> = clients
        .iter()
        .filter(|c| c.mapped)
        .map(|c| (c.class.clone(), c.title.clone()))        // L571 — BUG: uses mutable `class`
        .collect();
    if let Ok(Some(active)) = Client::get_active() {
        let key = (active.class.clone(), active.title.clone());  // L577 — BUG: uses mutable `class`
        if let Some(pos) = rows.iter().position(|r| *r == key) {
            rows.swap(0, pos);
        }
    }
    rows
}
```

## 2. The consistency target (notify path already correct)

`grep -nE '\.initial_class\.clone\(\)' src/platforms/hyprland.rs`:
- L398 `poll_window_state`: `app_class: active_window.initial_class.clone()`
- L479 `handle_window_state_change`: `app_class: active_window.initial_class.clone()`

These are the source of truth — what the firmware receives. The dialog (L571/L577)
is the only path still using mutable `class`. `grep '\.class\.clone()'` confirms
L571 + L577 are the ENTIRE fix surface (no other `.class.clone()` in the file).

## 3. Crate API confirmation (hyprland 0.4.0-beta.3)

`~/.cargo/registry/src/.../hyprland-0.4.0-beta.3/src/data/regular.rs`, `struct Client`:
- L239: `pub initial_class: String` — stable, set once at window creation.
- L241: `pub class: String` — mutable, can change at runtime (some Electron apps,
  or apps using `xprop`/`hyprctl setprop` to mutate WM_CLASS).

Both are `String` ⇒ `c.initial_class.clone()` / `active.initial_class.clone()` are
**type-identical** to the originals. The function signature is unchanged; no caller
is affected. The change cannot fail to compile (both fields exist on `Client`).

## 4. ⚠ The L571↔L577 coupling (the non-obvious correctness point)

L577's `key` is matched against L571's `rows`:
```rust
if let Some(pos) = rows.iter().position(|r| *r == key) { rows.swap(0, pos); }
```
`*r == key` compares `(row_class, row_title) == (key_class, key_title)`. If L571
uses `initial_class` but L577 uses `class` (or vice versa), then for any app where
`class != initial_class`:
- `row_class = initial_class`, `key_class = class` ⇒ `row_class != key_class` ⇒
  position lookup returns `None` ⇒ active window NOT moved to front.

⇒ **Both lines must change together.** Changing only one is a silent regression
(the dialog's active-window promotion breaks for differing apps). The contract
correctly names both lines; the PRP emphasizes the coupling.

## 5. Why no unit test

`list_foreground_windows()` calls `Clients::get()` and `Client::get_active()` —
hyprland-crate statics that do UNIX-socket IPC to a running Hyprland compositor.
- No injection point in the signature (takes no args; reads global IPC).
- No compositor in CI.
- Adding a seam (refactor to take clients/active as params) is outside the
  contract's two-swap scope.

⇒ Verification is **manual**, on a real Hyprland session, with an app where
`class != initial_class`. The structural correctness (field exists, compiles,
type-identical) is covered by `cargo build` + the grep gates.

## 6. Build matrix note

`hyprland` is a DEFAULT Cargo feature (`Cargo.toml`: `default = ["hyprland",
"macos", "linux-tray"]`). So `cargo build` / `cargo build --release` (default
features) compiles `src/platforms/hyprland.rs` — the edit IS checked by the normal
build gate. `cargo build --no-default-features` excludes it (trayless) and is
unaffected by this file's change.

## 7. Validation (verified shape)

- `cargo build` — zero warnings (type-identical swap).
- `cargo clippy --all-targets -- -D warnings` — clean.
- `cargo fmt --check` — exit 0.
- `cargo test --bin qmkonnect -- --test-threads=1` — all pass (no regression; the
  changed function has no unit tests).
- `grep '\.class\.clone()' src/platforms/hyprland.rs` → **zero** matches (both swapped).
- `grep '\.initial_class\.clone()' src/platforms/hyprland.rs` → **4** matches
  (L398, L479 notify unchanged; L571, L577 dialog fixed).
- `git diff` → exactly two `-`/`+` pairs, `class`→`initial_class` only.
- Manual (Hyprland): dialog class == notify-path app_class for a differing app;
  active window still promotes to top.

## 8. Scope boundaries (NOT this subtask)

- P1.M1.T2.S1: X11 `WM_CLASS` off-by-one (bug_findings Issue 2) — `src/platforms/x11.rs`.
- P1.M2.T3.S1: verify `docs/troubleshooting.md` window-class guidance wording.
- The notify paths (L398/L479) and the function signature are unchanged.
- `windows.rs`/`macos.rs` use their own class resolution — unaffected.