# Delta PRD — Remove mise/asdf as a Distribution Channel + Sync Docs

**Delta from:** PRD v0.2.8 snapshot #6 → #7 (`plan/007_…` → `plan/008_…`)
**Scope:** Documentation-only. **No source-code or packaging change required** — the code and specs are already in their final state.
**Size:** ~30 lines of PRD diff across 11 locations; this delta touches **1 source doc + 1 generated doc**.

---

## 1. What Changed in the PRD (diff summary)

The v0.2.8 → v0.2.8' delta makes two related decisions, but only **one** has any
remaining implementation work:

### 1.1 mise/asdf are NO LONGER a distribution channel (the dominant change)

Previously F15 listed "cross-platform **mise**/**asdf** version-manager
plugins" alongside AUR/Nix/.deb/.rpm/Homebrew/Scoop/Winget. The updated PRD
**removes mise/asdf from F15 entirely** and re-frames the channel list around
"channels that **run installer logic** (autostart + device wiring)." The
rationale is captured in PACKAGING.md §6.4 ("mise / asdf — NOT a channel
(category mismatch)"): an always-on single-instance tray daemon has no autostart
under a version manager, the "switch versions" workflow is forbidden by the
single-instance mutex, and updates would require re-wiring autostart.

This rewording appears in PRD §2.1 (Goal 6), §4 (F15 row), §5 (distribution
note), §12 (beta-status unaffected-channels list), and PACKAGING.md §1 header,
§6.2 (Scoop), §6.4 (replaced wholesale), §9 (asdf/mise CI job removed), and the
doc-map row.

### 1.2 New: self-install XDG autostart for binary-only Linux installs

The PRD adds a new bullet in LINUX.md §6.3 and PACKAGING.md §4.7:
`ensure_xdg_autostart(verbose)` writes the **user**
`~/.config/autostart/qmkonnect.desktop` on first run (gated by the marker
`~/.config/qmkonnect/.autostart_initialized`), so Scoop / cargo-binstall /
generic-tarball installs start at login without a package `postinst`. This is
what justifies treating Scoop as an acceptable (if weaker) channel despite not
running the Inno installer logic.

---

## 2. What Is Already Done (reference — do NOT re-implement)

Verification against the current codebase confirms **both decisions are fully
implemented in code and specs**:

| Item | State | Evidence |
|---|---|---|
| `ensure_xdg_autostart()` | ✅ implemented | `src/platforms/linux.rs:619`; called from `src/runners/linux.rs:39` |
| `packaging/asdf/` directory | ✅ removed | not present on disk |
| asdf/mise CI job | ✅ removed | no `asdf`/`mise` refs in `.github/workflows/release.yml` |
| `spec/PRD.md` | ✅ synced | F15 row + §2.1 + §5 say "mise/asdf NOT a channel" |
| `spec/PACKAGING.md` | ✅ synced | §6.4 = "mise / asdf — NOT a channel (category mismatch)"; §9 has no asdf job |
| `README.md` | ✅ synced | Package-Managers table has no mise/asdf row; caveat block cleaned |

**The previous session's `P1.M4` (asdf plugin milestone) and `P1.M5.T2.S2`
(asdf publish CI) are now obsolete** — their artifacts (`packaging/asdf/`, the
CI job) have already been removed. No task is needed to "undo" them.

---

## 3. What Still Needs Doing (the actual delta)

The **only** stale content is in the user-facing Jekyll docs:

### 3.1 `docs/installation.md` still advertises mise/asdf as channels

Three locations reference mise/asdf and link to the now-removed
`https://github.com/dabstractor/asdf-qmkonnect` plugin repo (broken links):

- **Line ~29** — the "mise / asdf are cross-platform version managers…"
  intro paragraph in the overview table block.
- **Lines ~289–301** — the Linux "mise / asdf" subsection with `asdf plugin add`
  / `mise install` command examples.
- **Lines ~367–376** — the macOS "mise / asdf — CLI only (no menu-bar tray)"
  subsection with `asdf` command examples.

These directly contradict the synced spec/PRD/PACKAGING decision and advertise a
removed channel with dead links. They must be removed (and the surrounding
sections left intact — the Nix/.deb/.rpm/AUR/Homebrew/Scoop/Winget coverage is
correct and stays).

### 3.2 `docs/llms_full.txt` is stale

The generated concatenation (`docs/generate_llms_full.sh` = README + docs/*.md)
still embeds the old mise/asdf content in two places: the README snapshot
(table row + caveat, lines ~157/160 — the live README is already clean) and the
docs/installation.md sections above (lines ~490/750–760/828–835). Regenerating
after fixing §3.1 resolves all of them in one shot.

---

## 4. Documentation Impact

This delta is **documentation-only**, so the doc work IS the deliverable:

- **Mode A (doc-with-work):** Not applicable in the usual sense — there is no
  code change to document. The single doc edit (`docs/installation.md`) is the
  work itself.
- **Mode B (changeset-level docs):** `docs/llms_full.txt` regeneration is the
  changeset-level sync — it must run after the installation.md edit. It depends
  on the edit task and is its own step.

No other docs (qmk-integration, configuration, usage, examples,
troubleshooting) reference mise/asdf (verified). `AGENTS.md` and
`REMAINING_ISSUES.md` are unaffected.

---

## 5. Implementation Plan

**One phase, one milestone, one task.** Proportional to a ~30-line doc-only
delta.

### Phase P1 — Documentation Sync: Remove mise/asdf Channel

#### Milestone P1.M1 — Remove mise/asdf from user-facing docs

##### Task P1.M1.T1 — Remove mise/asdf from docs/installation.md + regenerate llms_full.txt

**Subtask P1.M1.T1.S1 — Remove mise/asdf sections from `docs/installation.md`**
- Delete the "mise / asdf are cross-platform version managers…" intro paragraph
  (~line 29, just below the platform/channel overview table).
- Delete the Linux "mise / asdf" subsection (~lines 289–301: the paragraph +
  the `asdf plugin add` / `asdf install` / `mise plugin add` / `mise install`
  code block). Leave the surrounding Nix and .deb subsections intact.
- Delete the macOS "mise / asdf — CLI only (no menu-bar tray)" subsection
  (~lines 367–376: the paragraph + the `asdf` code block). Leave the Homebrew
  cask section and "Launch at login" section intact.
- Do **not** touch the Scoop, Winget, Homebrew, Nix, AUR, .deb, or .rpm
  content — those are correct. (Optional minor polish: the Scoop section's
  "the app writes the same HKCU `Run` value itself" already captures the
  self-heal behavior; no change required.)
- **Docs (Mode A):** this edit IS the doc work.

**Subtask P1.M1.T1.S2 — Regenerate `docs/llms_full.txt` + verify**
- Run `bash docs/generate_llms_full.sh` to regenerate the concatenation. This
  clears the stale README-snapshot mise/asdf lines (~157/160) and the stale
  installation.md lines (~490/750–760/828–835) in one pass.
- Verify: `grep -in 'mise\|asdf' docs/llms_full.txt` returns only innocuous
  hits (e.g. none expected; "promise" false-positives are in `docs/vendor/`
  which is not included by the generator).
- Verify: `grep -in 'asdf-qmkonnect' docs/installation.md docs/llms_full.txt`
  returns nothing (no dangling links to the removed plugin repo).
- **Docs (Mode B):** this IS the changeset-level regeneration step; depends on
  S1.

**No `cargo test` run is required** — this delta changes no Rust source. (If
desired for hygiene, `cargo test --bin qmkonnect -- --test-threads=1` can
confirm green, but it is not gated on this change.)

---

## 6. Success Criteria

1. `grep -in 'mise\|asdf' docs/installation.md` returns no hits (the three
   stale sections removed; Scoop/Winget/Homebrew/Nix/AUR/.deb/.rpm untouched).
2. No dangling `asdf-qmkonnect` repo links remain in `docs/`.
3. `docs/llms_full.txt` regenerated and free of mise/asdf channel references.
4. `spec/PRD.md`, `spec/PACKAGING.md`, `README.md`, source, and packaging
   already match the new PRD (no action — listed for completeness).

---

## 7. Out of Scope

- Any Rust source change (`ensure_xdg_autostart` is already implemented).
- Any `packaging/` change (`packaging/asdf/` already removed; CI job already
  removed).
- Any `spec/*.md` change (already synced).
- Any `README.md` change (already synced).
- Reversing the mise/asdf decision or adding a `cargo-binstall` metadata block
  (the PRD mentions cargo-binstall only as a *future* possibility in
  PACKAGING.md §6.4 — explicitly not this delta).