# Research Notes — P1.M2.T1.S3 (Linux tray save path handshake reset)

## Source verification (read against `src/linux_tray.rs` + `src/core/notifier.rs`)

### Contract corrections (verified against source)
1. **The helper is `current_config_vidpid()`, NOT `current_vidpid()`.**
   `grep -n "fn current_vidpid" src/linux_tray.rs` → **no match**. The real helper is
   `fn current_config_vidpid() -> (Option<u16>, Option<u16>)` at **L1006**. Returns the
   currently-configured VID/PID by reading the first existing config candidate
   (`get_config_paths()` → `parse_config()`), or `(None, None)` if no config yet.
   The task contract mislabeled it `current_vidpid()` / "L988-1012" — use
   `current_config_vidpid()`.

2. **`verbose` is NOT in scope** — same as siblings S1/S2. `save_and_notify(vendor_id,
   product_id)` (L718) takes only vid/pid. Its two call sites
   (L856 picker path `save_and_notify(Some(v), Some(p))`, L925 forms path
   `save_and_notify(vid, pid)`) pass no verbose either. → **Pass `false`** to
   `perform_handshake` (the bug_findings.md §132-sanctioned minimal choice). No
   signature change, no call-site edits.

### Exact anchors in `save_and_notify` (L718–748)
```
718  fn save_and_notify(vendor_id: Option<u16>, product_id: Option<u16>) {
719-724  vid_str / pid_str formatting
725      match write_config(vendor_id, product_id) {
726          Ok(path) => {                         ← reset+handshake block goes HERE (top)
727              let outcome = apply_device_rule(vendor_id, product_id);
728-741          detail = match outcome {...}; notify(...);
742          }
743          Err(e) => { eprintln!(...); notify(...); }
744      }
```
- Snapshot placement: **before** `match write_config` (~L725) — `write_config` overwrites
  config.toml, so `current_config_vidpid()` must run BEFORE it or it reads the NEW values.
- Reset+handshake placement: **top of the `Ok(path) =>` arm** (L726), before
  `apply_device_rule`. Guarded by `(vendor_id, product_id) != (old_vid, old_pid)`.

### Why this differs from S1/S2 (simpler!)
- S1/S2 have a `let mut merged = current_config;` MOVE (Config is Clone-not-Copy) → snapshot
  must precede the move.
- **S3 has NO such local.** `save_and_notify` receives the NEW vid/pid as params and reads
  OLD from the file via `current_config_vidpid()` (returns owned `Option<u16>`, Copy). So the
  snapshot is trivial: `let (old_vid, old_pid) = current_config_vidpid();` — no move hazard.

### Platform-gate / validation (THE key advantage over S1/S2)
- `src/main.rs:17` declares `mod linux_tray;` (Linux SNI tray). On the **Linux dev box this
  file IS compiled and type-checked** (unlike S1's `#[cfg(windows)]` / S2's `#[cfg(macos)]`
  which are cfg-gated out on Linux).
- Therefore `cargo build` + `cargo test --bin qmkonnect -- --test-threads=1` on this box ARE
  **definitive** for this edit — no platform-host caveat needed. Confidence boost vs S1/S2.

### Notifier functions (confirmed pub, fully-qualified → no `use`)
- `reset_handshake_state()` @ notifier.rs:814 — clears HOST_CAPABLE / BOARD_HAS_RULES /
  CALLBACK_NAMES, and sets `HAS_HANDSHAKED = false`.
- `perform_handshake(verbose: bool)` @ notifier.rs:353 → `perform_handshake_with`
  @ L509: idempotent guard `if HAS_HANDSHAKED.swap(true) { return }` (L511); reads
  `configured_filter()` FRESH (L83/521) so the just-written VID/PID selects the new board;
  drops the notifier lock per sweep iteration (L555). reset() clears HAS_HANDSHAKED first
  → perform_handshake RE-RUNS (order load-bearing).

### Testability
- `save_and_notify` is NOT unit-testable: it writes a real config (write_config →
  atomic_write to a platform config path), shells out to `apply_device_rule` (pkexec /
  udevadm / zenity `notify`), and calls the global handshake. Same as S1/S2 → **no new
  unit test**; verify via integration (two physical QMK boards) per bug_findings.md.
- Existing `linux_tray.rs` `mod tests` (L1169) covers pure helpers only
  (`status_text_uses_parity_glyphs`, `parse_id_handles_prefix_case_and_auto`,
  `color_scheme_parser_matches_spec`). Unchanged.

### Parallel-context coordination (S2 running concurrently)
- S2 edits `src/tray.rs` (macOS arm). S3 edits `src/linux_tray.rs`. **Different files, zero
  overlap.** Both call the same two `crate::core::notifier` fns (unchanged). No merge
  conflict possible.
- S1 (Windows, Complete) already established the `perform_handshake(false)` resolution and
  the two-insertion design; S3 mirrors it on the Linux save path.