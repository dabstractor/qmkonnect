# Documentation Audit — P1.M6.T1.S1 (changeset-level doc sync)

Scope: determine exactly which documentation files are stale after the P1.M1–P1.M5
changeset, and which are already accurate. Read-only audit (this agent does not edit).
All findings are git-verified.

## Changeset recap (what changed, user-visible vs internal)

| Module | Change | User-visible? |
|--------|--------|---------------|
| M1 | Debounce-worker panic race fixed + STATE/COND poison recovery | No (internal reliability) |
| M2 | config.toml / rules.toml writes now ATOMIC (temp+rename) on all platforms | No (file format unchanged) |
| M3 | Device-lifecycle handshake robustness (seed poll-thread initial state) | No (internal) |
| M4 | **Windows rules.toml-invalid notification: modal MessageBoxW → auto-dismissing WinRT TOAST** | **YES — the one user-facing change** |
| M5 | mtime-keyed config/rules read cache (hot-config preserved) | No (internal perf) |

## Finding 1 — `docs/troubleshooting.md`: ALREADY CORRECT (no edit needed)

The toast paragraph is present and accurate (committed in `17e4f6f` as part of
P1.M4.T1.S2). Lines 533–543:

> "At runtime, when `rules.toml` fails to parse during a window focus change,
> QMKonnect shows a **one-time desktop notification** … On **Windows** this is a
> **toast** that auto-dismisses after a few seconds and lands in Action Center
> (it is no longer a modal dialog you must click away); Linux uses `notify-send`
> and macOS uses a Notification Center alert. (On Windows the toast requires the
> installed Start Menu shortcut to render …)"

This matches the code (`src/platforms/mod.rs:189-192` → `show_toast(title, body)`).
No edit required; the implementing agent only VERIFIES it.

## Finding 2 — `docs/llms_full.txt`: STALE — MUST REGENERATE (primary deliverable)

- `llms_full.txt` is a **checked-in generated artifact** (`docs/generate_llms_full.sh`
  concatenates README.md + docs/*.md, stripping Jekyll front matter). It is NOT in
  `.gitignore` (verified). Script header: "Run after editing README.md or any docs/*.md".
- Last regenerated at commit `2e8f706 docs: regenerate llms_full.txt for unified [[rule]] schema`.
- Since then, ONLY `docs/troubleshooting.md` changed (`git diff --stat 2e8f706..HEAD -- README.md docs/`
  → `docs/troubleshooting.md | 10 ++++++++++`), and those 10 lines are exactly the toast paragraph.
- In the STALE `llms_full.txt`, the "rules.toml parse error" section (line 2548) jumps
  straight from "… for the full field table." to "### Device shows connected…" with NO
  notification paragraph in between (confirmed: `grep -niE "toast|action center" docs/llms_full.txt`
  returns nothing).
- **Action:** run `bash docs/generate_llms_full.sh`. The diff should add exactly the
  toast paragraph into the rules.toml-parse-error section. Verify with
  `grep -n "toast" docs/llms_full.txt` (expect 1+ hits where there were 0).

## Finding 3 — `README.md`: NO CHANGE (notifications not mentioned)

Read in full. The Features section lists: Cross-Platform Support, Core Functionality
(window changes / sends to keyboard / low resource / debug logging), Configuration,
Host-Side Window Rules. **No mention of Windows notifications, desktop alerts, or
toasts anywhere.** Per contract point (b): "If not mentioned, no change needed (the
feature is minor/edge-case)." → VERIFY only, do not add.

## Finding 4 — `docs/configuration.md`: NO CHANGE (format unchanged)

Documents the config.toml / rules.toml schema only. The M2 atomic-write change is a
write-path internals change — the file FORMAT is unchanged, so no doc edit. No claim
in configuration.md about write-safety, partial writes, or atomicity that is now wrong
(verified by reading). → VERIFY only.

## Finding 5 — `REMAINING_ISSUES.md`: 5 items STALE (verified fixed in code) — MARK RESOLVED

The bug-hunt report h2.2 validated #4, #5, #7, #13, #14 as "already fixed", but
REMAINING_ISSUES.md (dated "repo audit on 2026-07-09") still lists them as OPEN. Each
was code-verified during this audit:

| # | Item | Heading line | Code evidence (verified) |
|---|------|--------------|--------------------------|
| 4 | udev `/tmp`+`sudo` race + `MODE=0666` | 21 | Static rule `packaging/linux/udev/69-qmkonnect-rawhid.rules` is single-line, `ENV{ID_QMKONNECT}=="1"`-guarded, `MODE="0660"` + `TAG+="uaccess"` (no /tmp race, no world-writable node). `grep MODE src/platforms/linux.rs` → `0660`. |
| 5 | `static mut` data races | 31 | `src/platforms/windows.rs:22` + `src/platforms/macos.rs:43` comments: "Replacing the former `static mut` globals/VERBOSE". `grep -rn "static mut" src/platforms/{windows,macos}.rs` → only comments. |
| 7 | Hyprland backoff never resets | 40 | `src/platforms/hyprland.rs:26` "on its loss the backoff is reset to the initial value (#7)"; `:198-202` `delay_ms = INITIAL_RECONNECT_MS`. |
| 13 | macOS screen-recording hard-fail | 64 | `src/platforms/macos.rs:85-101` `ensure_screen_recording_permission()` does NOT block; runs app-name-only, redacts titles until granted. |
| 14 | X11 stub sending garbage | 67 | `src/platforms/x11.rs:25-53` real `xprop` (`_NET_ACTIVE_WINDOW` → `WM_CLASS`/`_NET_WM_NAME`); comment cites "issue #14". |

**Action:** for each, insert a one-line `> ✅ **Resolved.** …` note directly under the
heading (keep the original audit text intact for history). No new sections. The
implementing agent MUST re-run the grep verifications before marking (defense against
a regression). NOTE: these were fixed by PREVIOUS commits, not the P1.M1–M5 changeset
— the contract point (d) explicitly directs attention to them because h2.2 validated
them and the doc is stale.

## Finding 6 — No other stale references

`grep -rniE "messagebox|modal|blocking dialog" README.md docs/ REMAINING_ISSUES.md AGENTS.md`
(excluding llms_full.txt and ruby vendor) → only the (accurate) troubleshooting.md
toast paragraph. No doc claims writes are non-atomic/corruptible. No doc describes the
debounce behavior inaccurately. `AGENTS.md` screen-recording note is still accurate.

## Net deliverable set (minimal + accurate)

1. **EDIT** `docs/llms_full.txt` — regenerate via `bash docs/generate_llms_full.sh`.
2. **EDIT** `REMAINING_ISSUES.md` — mark #4, #5, #7, #13, #14 resolved (5 one-line notes).
3. **VERIFY (no edit)** — README.md, docs/troubleshooting.md, docs/configuration.md.
4. **SANITY** — `git diff --stat` shows ONLY the 2 doc files; ZERO source/packaging/Cargo; `cargo test` unchanged (no source touched).