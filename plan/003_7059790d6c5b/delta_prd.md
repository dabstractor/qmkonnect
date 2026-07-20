# Delta PRD — QMKonnect v0.2.4 → v0.2.8

**Scope:** The textual delta between the previous PRD (v0.2.4) and the current
PRD (v0.2.8). **Headline: the delta is already implemented in the codebase.**
This document records what changed, verifies compliance, and flags the one
residual cosmetic drift. It is intentionally short — proportional to the near-
zero remaining work.

---

## 1. What Actually Changed (diff analysis)

Five categories of change between the two PRD snapshots. Counting substantive
(not mechanical-rename) lines: ~3 behavioral changes + a pervasive-but-cosmetic
rename. The implementation for all five is **already present in the tree**.

### 1.1 Two-repo naming convention — SWAPPED (pervasive, cosmetic)
The underscore/hyphen assignment between the firmware C module and the Rust
transport crate was reversed:

| Project | v0.2.4 said | v0.2.8 says |
|---|---|---|
| QMK **firmware** C module | `qmk-notifier` (hyphen), repo `dabstractor/qmk-notifier` | `qmk_notifier` (underscore), repo `dabstractor/qmk_notifier` |
| Rust **transport crate** | `qmk_notifier` (underscore), repo `dabstractor/qmk_notifier`, v0.2.1 | `qmk-notifier` (hyphen), repo `dabstractor/qmk-notifier`, v0.3.0 |

v0.2.8 adds a clarifying note: the crate's package is `qmk-notifier` but Cargo
derives the library name `qmk_notifier` (hyphen→underscore), so Rust source still
reads `qmk_notifier::run`.

**Affected PRD sections:** §1.1 table + "Naming hazard" note, §1 diagram, §3,
§11, §12, §13 glossary; plus the ARCHITECTURE / PROTOCOL / FIRMWARE / HOST_RULES
companion specs (repo-layout comment, concurrency table, error model, crate
contract header/dep line, submodule URL, include paths, three-repo tables).

### 1.2 Cargo dependency declaration + version bump (behavioral, external)
The `Cargo.toml` dependency line changed shape and version:

```toml
# v0.2.4
qmk_notifier = { package = "qmk_notifier", git = ".../dabstractor/qmk_notifier", tag = "v0.2.1" }
# v0.2.8
qmk-notifier = { git = ".../dabstractor/qmk-notifier", tag = "v0.3.0" }
```

v0.3.0 is the typed-command transport release (the crate work tracked as
P1.M1 in the prior session). No explicit `package`/`rename` key is needed — the
repo's package is now literally `qmk-notifier`.

### 1.3 `Pattern::Single` matcher semantics — SIMPLIFIED (behavioral)
HOST_RULES.md §8(2) changed the `Single` matching rule:

- **v0.2.4:** "if the window has a title, match `p` against app_class only; else
  against the whole string."
- **v0.2.8:** "match `p` against **app_class only, always**. The title is never
  consulted for `Single`; use `Pattern::Parts(c, t)` to match the title."

Rationale (v0.2.8): firmware parity is "pattern with no GS vs. message with GS
matches the `msg_left` portion" — i.e. class-only — unconditionally.

### 1.4 Linux config/rules directory rename (behavioral)
The Linux (and macOS XDG + `/etc` fallback) config directory changed:

- **v0.2.4:** `~/.config/qmk-notifier/` (+ `/etc/qmk-notifier/`), framed as
  "historical; preserves existing installs."
- **v0.2.8:** `~/.config/qmkonnect/` (+ `/etc/qmkonnect/`), framed as "unified
  ahead of the first beta."

`rules.toml` rides in the same directory. This is a breaking change for any
pre-existing Linux install that wrote config under the old path.

### 1.5 Windows CI artifact path — WiX → Inno (behavioral)
PACKAGING.md §3.3 / §7: the `release.yml` Windows job switched from the WiX MSI
path (`build-installer.ps1`) to the Inno Setup path
(`packaging/windows/inno/build.ps1`), uploading
`QMKonnect-<ver>-windows-x64.exe` as the primary artifact. The legacy WiX files
remain on disk as an unused build path (explicitly OK per spec).

---

## 2. Compliance Status — Already Implemented (verified)

Every category above is already reflected in the production tree. **Do not
re-implement.** Evidence (spot-checked this session):

| Change | Status | Evidence |
|---|---|---|
| 1.1 Naming swap (source/docs) | ✅ done | `src/` imports `qmk_notifier::run` (correct alias); `docs/index.md`, `qmk-integration.md`, `troubleshooting.md`, `llms_full.txt` all use `qmk_notifier`=firmware / `qmk-notifier`=crate; local `/home/dustin/projects/qmk_notifier/` holds the firmware (`notifier.c`, `pattern_match.c`) |
| 1.2 Cargo dep + v0.3.0 | ✅ done | `Cargo.toml` pins `qmk-notifier = { git="…/dabstractor/qmk-notifier", tag="v0.3.0" }`; `Cargo.lock` resolves `qmk-notifier 0.3.0` (commit `f26893e`); `cargo check --offline` passes |
| 1.3 `Single` semantics | ✅ done | `src/core/pattern.rs` `match_pattern` → `Pattern::Single(p) => pattern_match(p, app_class, …)`; rustdoc states "title is deliberately NOT consulted" |
| 1.4 `qmkonnect/` paths | ✅ done | `src/platforms/linux.rs` (`get_config_paths`, `create_config_dir`), `src/platforms/macos.rs` (XDG+`/etc` fallbacks), `src/core/rules.rs` (`get_rules_paths`) all use `qmkonnect` |
| 1.5 CI Inno job | ✅ done | `.github/workflows/release.yml` installs Inno Setup 6, runs `packaging/windows/inno/build.ps1`, renames to `QMKonnect-<ver>-windows-x64.exe` |

The prior session's git history confirms this: `f3d06e8 Normalize crate naming
and update versions`, `730775e Standardize config path reference`, `fb19580 Port
firmware pattern matching engine`, `6e01f64 Complete documentation sync`.

---

## 3. Residual Drift (the only actionable work)

A systematic grep across `src/`, `docs/`, `Readme.md`, `Cargo.toml`,
`.github/`, and the companion specs found **one** cosmetic stale reference:

- **`docs/troubleshooting.md:647`** — in the firmware-debugging tip, the example
  of a non-existent callback name reads `` `qmk-notifier_notify` `` (hyphen),
  which is the *old* firmware naming. Under v0.2.8 the firmware module is
  `qmk_notifier` (underscore), so a user searching for a built-in callback would
  more plausibly look for `` `qmk_notifier_notify` ``. It is a throwaway example
  ("there is no built-in … callback"), so this is cosmetic, not functional.

No other stale references were found. The `package = "qmk_notifier"` /
`tag = "v0.2.1"` hits under `.pi-subagents/artifacts/` are cached prior-session
research outputs (gitignored, not shipped) — correctly ignored.

---

## 4. Documentation Impact

- **Mode A (doc-with-work):** the single change in §3 touches
  `docs/troubleshooting.md` only. No code symbol, no schema, no CLI surface
  changed — nothing else to document alongside work.
- **Mode B (changeset-level docs):** **none.** The v0.2.8 cross-cutting docs
  (README, configuration, qmk-integration, examples, troubleshooting,
  `llms_full.txt`) were already synced to the new naming/paths as part of the
  prior session's P6 milestone. No standalone final doc task is warranted.

---

## 5. Implementation Plan

One small task. No phases/milestones overhead is justified for a one-line
cosmetic fix plus a verification pass.

### Task D1 — Verify v0.2.8 compliance and fix the residual doc drift

- **D1.S1 — Fix the stale callback-name example in `docs/troubleshooting.md`.**
  Change the throwaway `` `qmk-notifier_notify` `` at line ~647 to
  `` `qmk_notifier_notify` `` (or rephrase to avoid a concrete invented name),
  so the example aligns with the v0.2.8 firmware naming (underscore). One-line
  edit; no behavior change.
- **D1.S2 — Confirm full-tree drift is clear (verification, no edits).** Re-run
  the targeted greps from this analysis against the whole repo
  (`qmk-notifier` *as firmware*, `qmk_notifier` *as crate repo/package*, the old
  `~/.config/qmk-notifier` path, `package = "qmk_notifier"`, `tag = "v0.2.1"`,
  WiX/`build-installer.ps1` in CI) excluding `.pi-subagents/` artifacts and the
  explicitly-retained `packaging/windows/installer.wxs` legacy file. Expect zero
  hits. Also confirm `cargo check --offline` still passes (already green this
  session).

**Acceptance:** the §3 grep returns clean (modulo the two documented
exceptions), `cargo check` is green, and `docs/troubleshooting.md` uses the
underscore firmware name in its callback example.

---

## 6. Risks / Notes

- **External repos are out of scope here.** The crate rename (→
  `dabstractor/qmk-notifier`, tag `v0.3.0`) and firmware rename (→
  `dabstractor/qmk_notifier`) were completed in the prior session (P1.M1 crate
  tag; firmware repo present locally). `Cargo.lock` already pins the resolved
  v0.3.0 commit, so builds are reproducible. No action required.
- **The Linux path rename (1.4) is breaking for pre-beta Linux installs** that
  wrote `~/.config/qmk-notifier/config.toml`. The PRD frames this as acceptable
  ("ahead of the first beta"). If a migration helper is later desired, it is a
  separate, larger task — **not** part of this delta.