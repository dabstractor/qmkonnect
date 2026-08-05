# Research Notes — P1.M2.T3.S2

**Task:** Verify `README.md` and `docs/installation.md` / `docs/usage.md` remain
accurate after the P1.M1.T3 (autostart quoting), P1.M2.T1 (handshake reset), and
P1.M2.T2 (title heuristic) changesets.

**Mode:** [Mode B] documentation-sync task — this IS the docs task. The contract
(point #3) defines a verification logic: confirm no text describes the Run-key value
*format* (quoted vs unquoted); confirm no README/overview text claims handshake or
title-filtering behavior the fixes contradict; if all accurate → "no doc changes
needed", mark complete.

---

## 1. What the three fixes actually changed (source of truth)

| Fix | Commit | Files | User-visible / documented surface? |
| --- | --- | --- | --- |
| **Autostart quoting** (P1.M1.T3) | `789dbc9` (app `autostart.rs`), `1f34529` (Inno `.iss`) | `src/autostart.rs`, `packaging/windows/inno/QMKonnect.iss` | **None.** Wraps the HKCU Run-key `REG_SZ` value in double-quotes. Pure correctness fix for spaced install paths. The mechanism (HKCU Run key), the toggle ("Open at Login"), and "enabled by default" are all unchanged. |
| **Handshake reset** (P1.M2.T1) | `e013d4d` (Windows), `1896c11` (macOS), `68aa7ea` (Linux) | `src/platforms/*/tray.rs` / `linux_tray.rs` | **None.** Adds `reset_handshake_state()` + `perform_handshake()` to the Settings VID/PID save path. Internal rebuild of the callback map on device change. No user-visible behavior, no toggle, no UI change. |
| **Title heuristic** (P1.M2.T2) | `d7c0a13` | `src/platforms/windows.rs` (`should_ignore_window`) | **None.** `.len()` → `.chars().count()`. Fixes UTF-8-byte vs Unicode-scalar inconsistency in the "very short title" ignore guard. Internal; the data *format* sent to the keyboard (`{application_class}{GS}{window_title}`) is unchanged. |

**Key insight:** All three are internal correctness fixes. None introduces,
removes, or renames a user-facing concept. The doc-sync question is narrowly:
*does any existing prose in the 3 target files make a claim the fixes falsify?*

---

## 2. The three target files — relevant lines (read this session, current tree)

### `README.md`
- **L278-282** (the contract-cited range, slightly re-anchored):
  ```
  - **Automatic Startup**: **Open at Login** via the HKCU `Run` key — default on,
    toggleable from the tray (`src/autostart.rs`).
  ```
  → Describes the **mechanism** (HKCU Run key) + **toggle** + "default on".
  **Does NOT describe the value FORMAT** (quoted vs unquoted). The quoting fix
  changes nothing about this sentence. ✅ accurate.
- **L219-221**: discovered-device picker "writes its VID/PID for you" — about the
  picker UI, **not** handshake-reset-on-settings-change. ✅ accurate, untouched.
- **L303**: `{application_class}{GS}{window_title}` data format — generic, unchanged
  by the heuristic (which only filters *which* windows to ignore). ✅ accurate.

### `docs/installation.md`
- **L30**:
  ```
  - Enables autostart via the HKCU `Run` key (toggle it in the tray: **Open at Login**)
  ```
  → Mechanism + toggle. **No format claim.** ✅ accurate.
- **L22**: "The installer launches QMKonnect and enables **Open at Login** by default."
  → "enabled by default" — unchanged by the quoting fix. ✅ accurate.

### `docs/usage.md`
- **L48** (Auto-Start on Boot → Windows):
  ```
  **Open at Login** is enabled by default. Toggle it from the system-tray icon → **Open at Login**.
  It's backed by the HKCU `Run` key (you can also disable it in Task Manager → Startup,
  but the tray toggle is the intended way).
  ```
  → Mechanism + toggle + "enabled by default". **No format claim.** ✅ accurate.

---

## 3. Negative-grep sweep — confirm no contradicting behavior claims

Ran against the 3 target files only (this is the contract scope):

```
handshake|reset_handshake|vid/pid|vid|pid change|multi-board   →  only README L219/221 picker prose (NOT handshake-reset)
title length|short title|ignore.*window|window.*filter|heuristic|byte length|character count  →  (none)
quoted|unquoted|quote|value.?data|reg_sz|format of the ... key  →  (none)
```

**Conclusion:** No prose in README.md / docs/installation.md / docs/usage.md claims:
- the Run-key value *format* (so the quoting fix cannot falsify anything);
- handshake behavior, VID/PID-change-reset, or multi-board state (so the handshake
  fix cannot falsify anything);
- window-title length filtering, byte/char counting, or any "ignore short title"
  rule (so the heuristic fix cannot falsify anything).

➡️ **Verdict: NO doc changes needed.** The task's primary path is a *verification
with zero file edits* — record the verdict in the completion/commit message and
mark complete.

---

## 4. Out-of-scope residual (flagged, NOT to fix here)

`docs/usage.md` "Auto-Start on Boot → macOS" (the "System Preferences → Users &
Groups → Login Items / Add QMKonnect.app" lines) is **stale** relative to
`docs/installation.md`, which correctly documents `SMAppService` + the "Launch at
Login" tray toggle (macOS 13+). **This is a PRE-EXISTING inconsistency, not caused
by any of the three fixes in this changeset** (the quoting fix is Windows Run-key
only; macOS autostart is a completely separate SMAppService mechanism). Per the
contract (point #3 = "verify the 3 fixes don't break accuracy"), fixing it would
**expand scope** beyond this changeset. **Do NOT fix it in this task.** Record it
as a residual for a future docs-hygiene task. (Also note: it is already tracked
implicitly — installation.md is the source of truth and explicitly says so.)

---

## 5. Sibling boundary — P1.M2.T3.S1 (docs/troubleshooting.md)

S1 owns the **only** doc edit in this milestone: one clarifying sentence in
`docs/troubleshooting.md` checklist item #3 (Hyprland `initial_class` vs X11
`WM_CLASS` class). **S2 must NOT touch `docs/troubleshooting.md`** — it is S1's
exclusive target. S2's scope is strictly `README.md`, `docs/installation.md`, and
`docs/usage.md`, and the verdict here is no-change. No overlap, no conflict.

Also out of scope for S2 (and unchanged by the 3 fixes): `docs/configuration.md`,
`docs/qmk-integration.md`, all `.rs` files, `PRD.md`, `tasks.json`,
`prd_snapshot.md`, `.gitignore`.

---

## 6. Validation reality (docs-only, no build gate)

There is no docs build / linter in the dev loop (AGENTS.md loops are cargo +
packaging). "Validation" for this task is **manual + read-only shell checks**:
- Negative greps re-run (confirm still no contradicting claims).
- `git diff --stat` is **empty** for the 3 target files in the no-change path
  (the only working-tree change in this milestone will be S1's
  `docs/troubleshooting.md` edit, which is NOT this task's to commit/revert).
- Do NOT run cargo or the packaging scripts for this task.