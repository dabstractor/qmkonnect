# Research Findings — P1.M2.T1.S2: macOS tray save path handshake reset

## Task
Add the SAME `reset_handshake_state()` + `perform_handshake()` handshake-reset-on-VID/PID-
change guard to the **macOS** Settings-dialog save path in `src/tray.rs`, mirroring the
Windows sibling P1.M2.T1.S1. This is the macOS half of Bug 4 (PRD ID 3): when a user with
two "capable" boards switches A→B in Settings, the save must reset + re-handshake so B's
callback name→id map is rebuilt (instead of B using A's stale map).

## Verdict: structurally identical to S1; `verbose` is also NOT in scope here → `false`

### `verbose` scope check (decisive — grep in the macOS region)
`grep -n "verbose" src/tray.rs | awk -F: '$1>=1566 && $1<=1920'` → **NO matches.** Neither
macOS settings function takes or closes over `verbose`:
- `fn show_macos_settings_dialog(config_path: ...) -> Result<...>` (L1589) — delegates
  (L1605) to `show_settings_dialog_with_pool`.
- `fn show_settings_dialog_with_pool(config_path: &std::path::Path) -> Result<...>` (L1648)
  — contains the actual save block.
- Shared caller `handle_settings_click` (L742, Win+macOS) → macOS branch (L792) calls
  `show_macos_settings_dialog`. No `verbose` anywhere in this chain.

➡️ **Use `perform_handshake(false)`** — exactly as bug_findings.md §132 prescribes ("verbose
is not in scope here — pass false or add a param") and exactly as the Windows sibling S1.
Do NOT thread `verbose` (would touch the shared `handle_settings_click`, conflicting with
S1/S3 scope).

## Exact save-block location (verified by read)

There is **exactly ONE save block on macOS** (one `atomic_write` @ L1901). It lives in
`show_settings_dialog_with_pool` (L1648), inside `unsafe { if response == 1000 { match (...)
{ (Ok(vid), Ok(pid)) => { … SAVE … } (Err,_)|(_,Err) => { error dialog } } } }`. Both macOS
functions are `#[cfg(target_os = "macos")]`.

```
L1648  fn show_settings_dialog_with_pool(config_path: &std::path::Path) -> Result<...>
L1649     current_config = parse_config(config_path)           // pre-save VID/PID source
L1867     let response: isize = msg_send![alert, runModal];    // BLOCKS tray thread
L1869     if response == 1000 {                                // OK
L1887       match (parse_id_field(vendor_str), parse_id_field(product_str)) {
L1891         (Ok(vid), Ok(pid)) => {                          // ← SAVE arm (20-space indent)
L1892           let mut merged = current_config;               // ← MOVES current_config (snapshot BEFORE)
L1893           if let Some((v, p)) = chosen { ... } else { merged.vendor_id = vid; merged.product_id = pid; }
L1900           let config_content = crate::core::render_config_body(&merged);
L1901           crate::core::atomic_write(config_path, &config_content)?;   // ← INSERT 2 AFTER this
L1902         }
L1903         (Err(e), _) | (_, Err(e)) => { show_macos_error_message(...); }   // no save → no handshake
L1905       }
L1906     }
L1907   Ok(())
```

**Indentation is 20 spaces** inside the `(Ok(vid), Ok(pid)) =>` arm. The two insertions go at
20-space indent, matching the surrounding `let mut merged` / `atomic_write` lines.

## Differences vs Windows (S1) — cosmetic, don't change the fix

| Aspect | Windows (S1) | macOS (S2) |
|---|---|---|
| Save dispatch | `if let Some(dr)=result { let mut merged… }` | `match (parse_id_field, parse_id_field) { (Ok,Ok)=>{…}, (Err,_)\|(_,Err)=>{err} }` |
| Merge branches | `dr.chosen` / `dr.manual` | `chosen` / direct `vid`/`pid` (chosen precedence) |
| `?`-on-write guards | yes (atomic_write `?`) | yes (atomic_write `?`) |
| Number of save blocks | 1 | 1 |
| `verbose` in scope? | NO → `false` | NO → `false` (grep-confirmed) |
| Platform gate | `#[cfg(windows)]` | `#[cfg(macos)]` |

The fix (snapshot-before-move + post-write diff-guarded reset+handshake) is **identical**.

## The two notifier functions (independently verified, same as S1)
- `pub fn perform_handshake(verbose: bool)` — `src/core/notifier.rs:353`. Idempotent via
  `HAS_HANDSHAKED.swap` guard; reads `configured_filter()` **fresh** (L521, L83) so the
  just-written VID/PID selects the new board; drops the notifier lock per sweep iteration.
- `pub fn reset_handshake_state()` — `src/core/notifier.rs:814`. Clears `HOST_CAPABLE` /
  `BOARD_HAS_RULES` / `CALLBACK_NAMES` / **`HAS_HANDSHAKED`** (so the immediately-following
  `perform_handshake` re-runs instead of no-oping on its guard).

**Order is load-bearing:** `reset_handshake_state()` FIRST (clears HAS_HANDSHAKED), THEN
`perform_handshake(false)` (whose guard would otherwise short-circuit). The existing device-
transition callsite mirrors this exact order: `tray.rs:455` (perform) / `:458` (reset).

## Config move semantics (same as S1)
`Config` derives `Clone` (NOT Copy) — `src/core/mod.rs:23`. `#[derive(..., Clone)] pub struct
Config`. So `let mut merged = current_config;` (L1892) **MOVES** `current_config`. `vendor_id`/
`product_id` are `Option<u16>` (Copy), so copying them out into `old_vid`/`old_pid` BEFORE L1892
is valid and required; reading `current_config.*` AFTER the move is a borrow-checker error.

## Platform gate — CANNOT compile on the Linux dev box
Both `show_macos_settings_dialog` and `show_settings_dialog_with_pool` are
`#[cfg(target_os = "macos")]`. On the Linux dev box, `cargo build`/`cargo test` compile only
the Linux path — the macOS save block is cfg-gated out and is NOT type-checked here. A green
Linux build is NOT proof the macOS edit compiles. Definitive validation requires a **macOS
host** (AGENTS.md macOS loop: `cd packaging/macos && ./clean.sh && ./build.sh && ./install.sh`
then `open /Applications/QMKonnect.app`). Cross-check to a macOS target from Linux also fails
(needs Apple frameworks / objc crate target).

This is the **same platform-gate situation** as the Windows sibling S1 — and the same split
between "Linux build = no-regression proof" and "target-host build = definitive".

## Scope / sibling boundaries
- **Edit ONE function's save arm only:** `show_settings_dialog_with_pool`'s `(Ok(vid),Ok(pid))`
  match arm. Do NOT touch `show_macos_settings_dialog` (it just delegates; no save there).
- Do NOT edit the Windows save path (S1) or `linux_tray.rs::save_and_notify` (S3).
- Do NOT change either macOS function's signature or thread `verbose`.
- No new imports (fully-qualified `crate::core::notifier::` paths).
- No new unit test (NSAlert `runModal` spawns a real Cocoa modal loop — not unit-testable; the
  existing `tray.rs` `mod tests` covers only pure helpers).
- P1.M2.T3 (docs sync, planned) and P1.M2.T2 (Windows title heuristic, planned) are unrelated.

## Pattern reference
- Existing device-transition callsite of the SAME pair: `src/tray.rs:455` (perform_handshake)
  and `:458` (reset_handshake_state) — inside `setup_tray`'s poll loop where `verbose` IS in
  scope. This task mirrors the pair from the Settings save path with `false` + the diff guard.
- Windows sibling PRP: `plan/006_8f4080956ee0/bugfix/001_558dab5d68a5/P1M2T1S1/PRP.md`.