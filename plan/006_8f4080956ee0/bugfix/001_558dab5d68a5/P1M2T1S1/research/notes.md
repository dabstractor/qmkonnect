# Research Notes — P1.M2.T1.S1: Add `reset_handshake_state()` + `perform_handshake()` to Windows tray save path

**Repo**: QMKonnect (`/home/dustin/projects/qmkonnect`) — Rust tray/menu-bar daemon.
**Target file**: `src/tray.rs` (ONE edit inside the Windows-only `show_settings_dialog`).
**Scope**: Windows save path only. macOS (`show_macos_settings_dialog` / `show_settings_dialog_with_pool`)
and Linux (`linux_tray.rs::save_and_notify`) are the **siblings** P1.M2.T1.S2/S3 — do NOT touch them.

---

## Parallel-context check (no conflict)

P1.M1.T3.S2 (in parallel) edits `packaging/windows/inno/QMKonnect.iss` — a packaging file,
completely unrelated to `src/tray.rs`. No overlap.

---

## The exact save path (verified current line numbers)

`show_settings_dialog(config_path: &Path)` — `src/tray.rs:838`, gated `#[cfg(target_os="windows")]`
at L824. The save block is the `if let Some(dr) = result { … }` arm, L968-989:

```rust
        if let Some(dr) = result {
            let mut merged = current_config;                 // L971 — MOVES current_config
            if let Some((v, p)) = dr.chosen {
                merged.vendor_id = Some(v);
                merged.product_id = Some(p);
            } else if let Some((v, p)) = dr.manual {
                merged.vendor_id = v;
                merged.product_id = p;
            }
            let config_content = crate::core::render_config_body(&merged);   // L979
            crate::core::atomic_write(config_path, &config_content)?;        // L981
            // Configuration saved successfully - no success dialog needed …
        }
```
- `current_config` is parsed at L849 (`crate::core::parse_config(config_path)`).
- `Config` derives only `Clone` (NOT `Copy`) — `src/core/mod.rs:12`. So `let mut merged = current_config;`
  **moves** `current_config`; it is INVALID after L971. ➡️ the VID/PID snapshot MUST precede L971.
  (`vendor_id`/`product_id` are `Option<u16>` which IS `Copy`, so copying them out before the move is valid.)

## The two notifier functions (both `pub`, fully-qualified paths — no `use` needed)

- `reset_handshake_state()` — `src/core/notifier.rs:814`. Clears `HOST_CAPABLE`, `BOARD_HAS_RULES`,
  `CALLBACK_NAMES` (`.clear()`), `HAS_HANDSHAKED` (all → false/empty). `#[cfg_attr(not(any(target_os=
  "macos","windows")), allow(dead_code))]` → live on Windows, no dead-code warning. ✓
- `perform_handshake(verbose: bool)` — `src/core/notifier.rs:353` → delegates to `perform_handshake_with`.
  - Idempotent guard: `if HAS_HANDSHAKED.swap(true, SeqCst) { return; }` (L511). Because we call
    `reset_handshake_state()` FIRST (sets HAS_HANDSHAKED=false), the swap returns false → handshake RUNS. ✓
  - Reads `configured_filter()` FRESH (L525) — so it picks up the VID/PID just written by `atomic_write`. ✓
  - Locking: acquires `notifier.lock()` for QueryInfo+SetOs, then `drop(n)` BEFORE the callback sweep (L555,
    the #4 contention fix) and re-acquires per iteration. No long-held lock → calling from the tray thread
    does NOT starve window notifications and cannot deadlock (tray thread holds no notifier lock). ✓
  - Bounded: `CALLBACK_SWEEP_DEADLINE` caps a misbehaving board; a real board answers in ~ms.

## ⚠️ THE `verbose` PROBLEM — contract is WRONG; bug-findings is RIGHT

- **Task contract LOGIC #3 says:** "perform_handshake(verbose). The `verbose` variable is in scope in the
  tray save function." — **FALSE.** `show_settings_dialog(config_path)` has NO `verbose` param, and its
  caller `handle_settings_click()` (L742, shared Win+macOS) has NO `verbose` param either. `verbose` exists
  only in `setup_tray(verbose: bool)` (L298) and the poll loop (L453/455).
- **bug_findings.md line 132 (authoritative architecture research) says:** *"`verbose` is not in scope here —
  pass `false` or add a param."* — **CORRECT** (verified against the source).

### Resolution chosen for this PRP: `perform_handshake(false)` (primary)
- Minimal: zero signature changes, zero ripple. The diff stays inside the ONE Windows save block —
  exactly the task's Windows-only scope.
- No sibling conflict: does NOT touch the shared `handle_settings_click` (which would bleed into the macOS
  sibling P1.M2.T1.S2) nor `linux_tray.rs` (sibling S3).
- Cost is only the loss of `eprintln!` debug logging on THIS one user-initiated re-handshake. The poll-thread
  handshake (L455) still uses the real `verbose`. Sanctioned by bug_findings.md.
- Alternative (documented, NOT recommended for THIS task): thread `verbose` through
  `show_settings_dialog(config_path, verbose)` ← `handle_settings_click(verbose)` ← caller L540. This is
  correct but touches the shared `handle_settings_click` (affects macOS path) → sibling-scope conflict. Avoid.

## Threading model of the call (synchronous, on the tray event-loop thread)

- The Settings menu click (L540) → `handle_settings_click()` → `show_settings_dialog()` all run ON the tray
  event-loop thread; the modal `GetMessageW` loop (L947) blocks that thread while the dialog is open.
- After the user clicks OK, the dialog loop exits; we're still on the tray thread. Calling
  `perform_handshake(false)` synchronously blocks the tray thread for the handshake duration (~ms for a real
  board). Acceptable: the modal dialog just closed, the tray thread has no real-time duty, and the sweep
  releases the notifier lock per iteration. The debouncer/window monitors run on separate threads.
- Do NOT spawn a thread (contract + bug-findings both specify a synchronous call; adding a thread is scope
  creep + error-handling complexity).

## ⚠️ PLATFORM-GATE GOTCHA — the edit CANNOT be compiled/checked on Linux

- The edited function is `#[cfg(target_os = "windows")]` (L824). On the Linux dev box, `cargo build` /
  `cargo test` compile ONLY the Linux path — the Windows `show_settings_dialog` body is **cfg-gated out and
  NOT type-checked here**. An implementer who runs `cargo build` on Linux and sees green has NOT validated
  their Windows edit.
- Cross-check attempted: `cargo check --target x86_64-pc-windows-msvc` FAILS on this box — the `eventlog`
  crate's build.rs needs `x86_64-w64-mingw32-windmc`/`windres` (mingw tools absent). So the windows-msvc
  target is NOT buildable on Linux (matches AGENTS.md: MSVC link needs the Windows toolchain).
- ➡️ **Definitive validation is on a Windows host** (AGENTS.md Windows dev loop: `cargo build` then
  `cargo test --bin qmkonnect -- --test-threads=1`). On Linux: `cargo build` + `cargo test` only prove
  "no regression in the Linux build" — pair with rigorous textual review of the exact diff lines.

## The exact edit (2 insertion points, both inside the `if let Some(dr) = result` block)

```rust
            // ── INSERT (before `let mut merged = current_config;` at L971) ──
            // Snapshot pre-save VID/PID BEFORE the move into `merged` (Config is
            // Clone, not Copy, so `merged` consumes current_config).
            let old_vid = current_config.vendor_id;
            let old_pid = current_config.product_id;
            // ── END INSERT ──
            let mut merged = current_config;
            if let Some((v, p)) = dr.chosen {
                merged.vendor_id = Some(v);
                merged.product_id = Some(p);
            } else if let Some((v, p)) = dr.manual {
                merged.vendor_id = v;
                merged.product_id = p;
            }
            let config_content = crate::core::render_config_body(&merged);
            crate::core::atomic_write(config_path, &config_content)?;

            // ── INSERT (after atomic_write succeeds) ──
            // Bug 4 (PRD ID 3): a VID/PID change selects a different board, so the
            // global CALLBACK_NAMES map (built for the OLD board) is stale. Reset
            // + re-handshake immediately so the new board's name→id map is rebuilt
            // (no replug needed). reset clears HAS_HANDSHAKED so perform_handshake
            // re-runs (its idempotent guard). perform_handshake reads config.toml
            // fresh (configured_filter), so the just-written VID/PID takes effect.
            if merged.vendor_id != old_vid || merged.product_id != old_pid {
                crate::core::notifier::reset_handshake_state();
                crate::core::notifier::perform_handshake(false);
            }
            // ── END INSERT ──
```

## Validation (verified)

```bash
# Linux (this box): NO regression in the Linux build/tests. (Does NOT typecheck the Windows edit.)
cargo build
cargo test --bin qmkonnect -- --test-threads=1   # AGENTS.md: single-threaded (shared debouncer state)

# Windows host (DEFINITIVE — the only place the #[cfg(windows)] edit compiles): per AGENTS.md
cargo build
cargo test --bin qmkonnect -- --test-threads=1
```
- No unit test is added (the Win32 dialog spawns a real message loop — not unit-testable; the existing
  tray.rs `mod tests` at L2984 only covers pure helpers like `device_status_text`). Manual verification:
  two capable boards A+B → handshake A → Settings → pick B → save → B's callback map is now live.