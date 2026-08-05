# PRP — P4.M1.T1.S1: Config template `0xFEED` cleanup + `--list-devices` kind column

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. **CODE-only task across three files:**
> (1) `src/core/mod.rs` — replace the `0xfeed`/`0x0000` literals in BOTH config
> renderers (`render_default_config_template` + `render_config_body`'s `None`
> arms) with `0x????` + the spec §7.2 comment; (2) `src/core/notifier.rs` — give
> `list_devices()` a `kind` column sourced from a one-shot `classify_devices()`
> pass (signature `list_devices(verbose: bool)`); (3) `src/main.rs` — thread
> `verbose` into the dispatch. Plus 2 unit tests. **Consumes**
> `classify_devices(verbose) -> Vec<ClassifiedDevice>` + `DeviceKind`/
> `ClassifiedDevice` (P3.M1.T1.S2 — Complete, in-tree + verified).
>
> **SCOPE WALL (critical):** this task does NOT touch `README.md`,
> `docs/configuration.md`, `docs/llms_full.txt`, any `spec/*.md`, `Cargo.toml`,
> `src/tray.rs`, or `src/linux_tray.rs`. The `0xfeed` literals that ALSO appear in
> README/docs (per `plan/005_*/architecture/config_cli.md`) are owned by the
> **doc-sync** siblings **P4.M1.T1.S2** (Mode-A user-facing doc sync) and
> **P4.M2.T1.S1** (README audit + regenerate `llms_full.txt`). Contract point 5:
> "[Mode A] The template comment IS the user-facing doc here; no separate
> docs/*.md edit for the literal." See G8.
>
> **PARALLEL-SAFETY:** P3.M2.T1.S3 (Linux zenity picker) is being implemented
> concurrently and touches `src/linux_tray.rs` ONLY. This task touches
> `src/core/mod.rs` + `src/core/notifier.rs` + `src/main.rs` — fully disjoint
> files; no merge conflict and no shared symbol. P3.M2.T1.S1/S2 (Windows/macOS
> pickers) are Complete and also don't touch these three files.

---

## Goal

**Feature Goal**: (a) Remove every `0xfeed`/`0x0000` literal from the two
config-body renderers in `src/core/mod.rs` — `render_default_config_template`
(the `-c` seeder / first-run template) and `render_config_body`'s `None` match
arms (the Settings-save renderer) — replacing each with `0x????` and the comment
`# unset: auto-discover any QMK keyboard (recommended)` (verbatim from
`spec/DEVICE_DISCOVERY.md` §7.2), so a freshly-seeded / re-saved `config.toml`
no longer carries the literal that users historically misread as "0xFEED is the
default." (b) Give `--list-devices` a `kind` column (`qmk_notifier` for a
`Capable` board, `qmk-only` for a `NotQmkNotifier` board, `-` otherwise) sourced
from a one-shot `classify_devices(verbose)` pass keyed by the stable hidapi
`path`, so the VID/PID discovery tool also reports each board's qmk_notifier
capability.

**Deliverable** (additive edits across 3 files):
1. **`src/core/mod.rs`** — 4 literal replacements (2 in
   `render_default_config_template` lines 238-239; 2 in `render_config_body`'s
   `None` arms line 258 + ~261) → `0x????` + the §7.2 comment. PLUS 1 new test
   `template_has_no_0xfeed_literal` (the §9 gate: rendered template + save-renderer
   `None` body contain no `0xfeed` and DO contain `0x????`).
2. **`src/core/notifier.rs`** — `list_devices()` → `list_devices(verbose: bool)`;
   body gains a path-keyed `HashMap<String, DeviceKind>` from
   `classify_devices(verbose)`, a `kind` column in the header + each row
   (`Capable`⇒`qmk_notifier`, `NotQmkNotifier`⇒`qmk-only`, unmatched⇒`-`), and a
   pure `kind_label(&DeviceKind) -> &'static str` helper + Mode-A doc-comment.
   PLUS 1 new test `kind_label_matches_spec`.
3. **`src/main.rs`** — 1-line dispatch update: `list_devices()?` →
   `list_devices(verbose)?` (line ~117; `verbose` is already in scope).
4. **NO other files change** (G8 scope wall).

**Success Definition**:
- `render_default_config_template()` output contains NO `0xfeed` and DOES contain
  `0x????`; still parses (via `toml::from_str`) to all-`None` device fields +
  default timing (the `0x????` line is commented, never parsed).
- `render_config_body(&Config::default())` (all `None`) output contains NO
  `0xfeed` and DOES contain `0x????`; the `Some(v)`/`Some(p)` arms are UNCHANGED
  (still `vendor_id = 0x{v:04x}`).
- `qmkonnect --list-devices` prints a header ending in `…product  kind):` and each
  row has a 6th field that is `qmk_notifier` (capable Tier-1 board), `qmk-only`
  (QMK raw-HID board without the module), or `-` (everything else / unmatched /
  classify failure). With no HID devices at all, only the header prints (0 rows).
  With Tier-1 boards but HID-probe failure, every kind cell is `-` (no panic).
- `qmkonnect -v --list-devices` additionally prints `classify_devices`'s
  per-candidate probe diagnostics to **stderr** (the **stdout** table stays clean).
- `cargo build --bin qmkonnect` clean (no NEW warnings); `cargo test --bin
  qmkonnect -- --test-threads=1` green (existing tests unchanged + the 2 new
  tests). `git diff --stat` = `src/core/mod.rs` + `src/core/notifier.rs` +
  `src/main.rs` only.

## User Persona (if applicable)

**Target User**: a user setting up QMKonnect who (a) reads the seeded
`config.toml` to understand what to set, and (b) runs `qmkonnect --list-devices`
to find their keyboard's VID/PID and now also to see whether the board they flashed
is actually qmk_notifier-capable (vs a VIA/Vial-only QMK board that won't respond).

**Use Case**: user flashes `qmk_notifier` onto a board, runs
`qmkonnect --list-devices`, and sees `…  Dactyl  qmk_notifier` — confirming the
module is live. A second QMK board (VIA-only) shows `…  Keychron  qmk-only`,
explaining why it won't receive notifications. Separately, the seeded
`config.toml` they open shows `# vendor_id = 0x????   # unset: auto-discover any
QMK keyboard (recommended)` — no longer the misleading `0xfeed`.

**Pain Points Addressed**: (1) the historical misreading that `0xFEED` is the
default VID (it's a matching-dead crate fallback; `None` = wildcard in QMKonnect);
(2) `--list-devices` listed raw VID/PID/usage with no indication of which boards
are actually qmk_notifier-capable, forcing the user to guess / trial-and-error.

## Why

- **`spec/DEVICE_DISCOVERY.md` §7.2** mandates the `0xFEED` comment cleanup: the
  literal "has historically been misread as '0xFEED is the default.'" The target
  text (§7.2 lines 380-381) is reproduced verbatim in this PRP's tasks. The
  crate's `DEFAULT_VENDOR_ID = 0xFEED` / `DEFAULT_PRODUCT_ID = 0x0000` are
  explicitly "matching-dead" (`PROTOCOL.md` §3.3) — `None` always means wildcard
  in QMKonnect.
- **`spec/DEVICE_DISCOVERY.md` §8** (Implementation Map, CLI row) mandates the
  kind column: "`--list-devices` output gains a `kind` column (`qmk_notifier` /
  `qmk-only` / `via-only`-ish, from a one-shot `classify_devices`)" in
  `src/core/notifier.rs` + `src/main.rs`.
- **`spec/DEVICE_DISCOVERY.md` §9** (Testing Plan) pins the gate test: "**`0xFEED`
  cleanup:** the seeded template contains no literal `0xfeed`."
- **`spec/CONFIG.md` §2** documents the cleanup ("the seeded template … and the
  `None` rendering of `vendor_id` no longer carry the literal `0xfeed` — they read
  `0x????`") — this task implements exactly that for the CODE renderers.
- **Unblocks the doc-sync siblings** by leaving the code renderers clean for
  P4.M1.T1.S2 / P4.M2.T1.S1 to mirror in README/docs.

## What

Mechanical, well-bounded code edits: 4 literal replacements in `mod.rs`, a small
`list_devices` extension + pure helper in `notifier.rs`, a 1-line dispatch update
in `main.rs`, and 2 pure unit tests. No schema change, no new deps, no CLI-flag
addition (`--list-devices` already exists), no tray/UI change, no docs/spec edits.

### Success Criteria
- [ ] **`render_default_config_template`** (mod.rs:238-239) — the two
      `# vendor_id = 0xfeed` / `# product_id = 0x0000` lines become
      `# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)`
      and the matching `product_id` line (EXACT §7.2 text).
- [ ] **`render_config_body` `None` arms** (mod.rs:258 + ~261) — the SAME `0x????` +
      §7.2 comment text in BOTH the `vendor_id` and `product_id` `None` arms. The
      `Some(v)`/`Some(p)` arms are UNCHANGED.
- [ ] **`list_devices(verbose: bool)`** (notifier.rs:129) — new signature; body
      builds `HashMap<String, DeviceKind>` from `classify_devices(verbose)` keyed
      by `ClassifiedDevice.path`, prints a header ending `…product  kind):`, and
      each row appends `kind_label(...)` or `"-"` (lookup by
      `d.path().to_string_lossy().to_string()`).
- [ ] **`kind_label(&DeviceKind) -> &'static str`** — `Capable{..}` ⇒
      `"qmk_notifier"`, `NotQmkNotifier` ⇒ `"qmk-only"`. Pure; unit-tested.
- [ ] **`src/main.rs`** dispatch (line ~117): `list_devices(verbose)?` (was
      `list_devices()?`).
- [ ] **Mode-A doc-comment** on `list_devices` citing `spec/DEVICE_DISCOVERY.md`
      §8 + §7.2; notes the path-keyed classify pass + the filter-scope caveat.
- [ ] **`template_has_no_0xfeed_literal`** (mod.rs test): asserts
      `render_default_config_template()` AND `render_config_body(&Config::default())`
      contain no `0xfeed` and DO contain `0x????` (the §9 gate).
- [ ] **`kind_label_matches_spec`** (notifier.rs test): both `DeviceKind` variants
      map to the §8 labels.
- [ ] `cargo build --bin qmkonnect` clean; `cargo test --bin qmkonnect --
      --test-threads=1` green; `git diff --stat` = the 3 source files only.

## All Needed Context

### Context Completeness Check

_Pass_: an agent with no prior knowledge can implement this using only this PRP,
because: (a) the EXACT current text of both renderers (verbatim, with grep-confirmed
line numbers 238-239 + 258/~261) and the EXACT §7.2 target text are reproduced;
(b) the EXACT current `list_devices()` body + the consumed `classify_devices`/
`DeviceKind`/`ClassifiedDevice` signatures (incl. the path-keying fact from
`enumerate_candidates` @992) are reproduced; (c) the verified fact that NO existing
test asserts the literal `0xfeed` template string (so none need "fixing") is
documented with the exhaustive-grep table — the implementer won't hunt for a
non-existent test; (d) the only call site of `list_devices` (main.rs:117) is
named, so the signature change is a safe 1-site update; (e) the kind-column labels
+ the `-` placeholder + the path-keyed HashMap design + the clean-degradation
behavior (empty map ⇒ all `-`, no panic) are all specified; (f) the verbose-on-
stderr/table-on-stdout separation is explained; (g) 10 gotchas pinned (G1-G10);
(h) the scope wall (no docs/spec/README/Cargo/tray this task) is explicit.

### Documentation & References

```yaml
# MUST READ — the spec source of truth (the cleanup target + the kind-column mandate)
- file: spec/DEVICE_DISCOVERY.md
  why: "§7.2 (lines 373-390) gives the EXACT target text for the 0xFEED cleanup
        (# vendor_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)).
        §8 (Implementation Map, CLI row) mandates the kind column (qmk_notifier /
        qmk-only, from a one-shot classify_devices) in src/core/notifier.rs +
        src/main.rs. §9 (Testing Plan) pins the gate test: 'the seeded template
        contains no literal 0xfeed'. §2.3 defines classify_devices/ClassifiedDevice."
  section: "## 7.2 0xFEED comment cleanup  +  ## 8 Implementation Map (CLI row)  +  ## 9 Testing Plan"
  gotcha: "§7.2 only NAMES render_default_config_template, but render_config_body's
           None arms emit the SAME literal — BOTH must change (G1). The crate's
           DEFAULT_VENDOR_ID=0xFEED is matching-dead (don't touch the crate)."

# MUST READ — the CONFIG spec (the §2 cleanup note this task implements in code)
- file: spec/CONFIG.md
  why: "§2 documents the cleanup: 'the seeded template (render_default_config_template)
        and the None rendering of vendor_id no longer carry the literal 0xfeed —
        they read 0x????'. §1 confirms vendor_id/product_id stay Option<u16> (None
        = match any) and legacy files with 0xfeed keep working (Some(0xfeed)) — so
        0xfeed REMAINS a valid config VALUE (just not the template literal)."
  section: "## 2. The Shared Config-Body Renderer (render_config_body)  +  the §2 0xFEED cleanup note"

# MUST READ — the verbatim research (THIS task's exact edit sites + the no-test-breaks proof)
- file: plan/005_8b95ea464bd9/P4M1T1S1/research/notes.md
  why: "§0 the verbatim current renderers (lines 238-239 + 258/~261) + the §7.2
        target. §1 the exhaustive-grep PROOF that no existing test asserts the
        literal 0xfeed template text (the round-trip tests assert parsed field
        values; 0xfeed stays a valid config value). §2 list_devices current body +
        the classify_devices/DeviceKind/ClassifiedDevice API + the path-keying
        fact (enumerate_candidates @992). §3 the 5 design decisions (D1-D5). §4
        the 10 gotchas (G1-G10). §5 confidence."

# MUST READ — the files THIS task edits
- file: src/core/mod.rs
  why: "render_default_config_template (the 0xfeed literal @238-239, inside the fn
        whose doc-comment starts ~line 218) and render_config_body (the None arms
        @258 + ~261). The round-trip tests @510 (render_default_config_template_round_trips_to_defaults)
        and @524 (render_config_body_round_trips) assert PARSED field values, not
        literal strings — they still pass; ADD template_has_no_0xfeed_literal near
        them. Config @23-48 (vendor_id/product_id: Option<u16>) is UNCHANGED."
  pattern: "the two renderers are pure String builders; the None arms of
            render_config_body are the save-path render (every Settings save calls
            it). The template is a single concatenated &str literal (mind the
            trailing \\ + the alignment spaces after the field name)."
  gotcha: "G1 BOTH renderers. G10 the new 0x???? line is COMMENTED (never parsed);
           don't make it an uncommented value (0x???? is not valid TOML hex)."
- file: src/core/notifier.rs
  why: "list_devices @129 (the fn to extend; only call site is main.rs:117).
        classify_devices @1116 (CONSUMED — Vec<ClassifiedDevice>). DeviceKind @818
        (Capable{..}/NotQmkNotifier; #[derive(Clone)]). ClassifiedDevice @843
        (path:String = d.path().to_string_lossy().to_string() per enumerate_candidates
        @992; vendor_id/product_id:u16; kind:DeviceKind). ADD kind_label + the
        kind_label_matches_spec test in the existing #[cfg(test)] mod tests."
  pattern: "enumerate_candidates (@992) maps d.path() → Candidate.path the SAME way
            list_devices iterates d.path(); so the kind-column lookup is a path-
            keyed HashMap. classify_devices returns Vec::new() on HID error ⇒
            clean degradation to all '-' cells."
  gotcha: "G3 path-keyed (NOT vid/pid — splits share vid/pid). G4 classify_devices
           is filter-scoped (configured_filter); narrowed VID/PID ⇒ non-matching
           boards show '-'. G5 empty map ⇒ all '-', no panic."
- file: src/main.rs
  why: "the --list-devices dispatch @115-119 (crate::core::notifier::list_devices()?).
        verbose is captured once near the top of the args fn and is in scope here
        (used later by list_callbacks(verbose)/validate_rules(...,verbose)). Change
        to list_devices(verbose)?."
  gotcha: "the ONLY call site of list_devices — the signature change is a safe
           1-line update. No other caller, no existing test for list_devices."

# MUST READ — the predecessor classification API (consumed; Complete + tested)
- file: src/core/notifier.rs   # (same file; these are the CONSUMED symbols)
  why: "classify_devices(verbose: bool) -> Vec<ClassifiedDevice> @1116 — Tier-1
        enumerate + per-candidate QUERY_INFO + 5s-TTL cache; verbose=true ⇒ eprintln!
        probe diagnostics (stderr). The kind column calls it ONCE and keys by path."

# Reference — hidapi DeviceInfo::path() (the kind-column lookup key)
- url: https://docs.rs/hidapi/latest/hidapi/struct.DeviceInfo.html
  why: "DeviceInfo::path() returns the stable OS device path (the same value
        enumerate_candidates @992 stringifies into ClassifiedDevice.path). The two
        HidApi snapshots (list_devices's and classify_devices's) yield identical
        path strings ⇒ the HashMap join is correct. device_list() borrows from the
        HidApi so the loop must run while `api` is alive (it does)."
  critical: "do NOT key the kind lookup by (vid,pid,usage_page,usage) — split
             keyboards share that tuple. Path is the unique stable key (G3)."

# Reference — the architecture doc (the 0xFEED location table + CLI dispatch map)
- file: plan/005_8b95ea464bd9/architecture/config_cli.md
  why: "the 0xFEED Locations table (confirms BOTH code renderers + the README/docs
        sites owned by the doc-sync siblings) + the CLI dispatch table (--list-devices
        @115-119) + list_devices() current body. USE THIS to confirm scope: this
        task owns the mod.rs code sites ONLY; the README/docs sites are NOT this task."
```

### Current Codebase tree (relevant subset)

```bash
src/
  core/
    mod.rs             # Config @23-48 (Option<u16> fields, UNCHANGED);
                         # render_default_config_template (0xfeed @238-239);
                         # render_config_body (None arms 0xfeed @258 + ~261);
                         # tests @506+ (round-trip @510/@524 still pass; ADD the
                         #   template_has_no_0xfeed_literal gate test here).
                         # <-- THIS TASK: 4 literal replacements + 1 test.
    notifier.rs        # list_devices @129 (extend + signature change);
                         # classify_devices @1116, DeviceKind @818, ClassifiedDevice
                         #   @843, enumerate_candidates @992 (CONSUMED, read-only);
                         # tests (ADD kind_label_matches_spec in #[cfg(test)] mod tests).
                         # <-- THIS TASK: list_devices(verbose) + kind_label + 1 test.
  main.rs              # --list-devices dispatch @115-119 (list_devices()? → list_devices(verbose)?).
                         # <-- THIS TASK: 1-line dispatch update.
spec/
  DEVICE_DISCOVERY.md  # §7.2 (target text) + §8 (kind column) + §9 (gate test). READ-ONLY.
  CONFIG.md            # §2 (cleanup note). READ-ONLY.
plan/005_*/architecture/config_cli.md   # 0xFEED location table + CLI map. READ-ONLY.
# NOT touched this task: README.md, docs/*.md, Cargo.toml, tray.rs, linux_tray.rs (G8)
```

### Desired Codebase tree with files to be added/changed

```bash
src/
  core/
    mod.rs             # MODIFIED — 4 literal replacements (0xfeed/0x0000 → 0x???? +
                         #   §7.2 comment, in BOTH renderers) + 1 test
                         #   (template_has_no_0xfeed_literal).
    notifier.rs        # MODIFIED — list_devices() → list_devices(verbose: bool) with
                         #   a path-keyed kind column + kind_label() pure helper +
                         #   Mode-A doc-comment + 1 test (kind_label_matches_spec).
  main.rs              # MODIFIED — 1 line: list_devices()? → list_devices(verbose)?.
    # EVERYTHING else unchanged (Cargo.toml, tray.rs, linux_tray.rs, platforms/*,
    # spec/*, docs/*, README.md, packaging/*)
```

### Known Gotchas of our codebase & Library Quirks

```rust
// CRITICAL (G1 — BOTH renderers): the PRD §7.2 only NAMES render_default_config_template,
//   but render_config_body's None arms emit the SAME 0xfeed/0x0000 literal. The save
//   path (every Settings-dialog save) calls render_config_body — updating ONLY the
//   template would leave saves re-injecting 0xfeed. UPDATE BOTH (mod.rs:238-239 AND
//   258 + ~261). Always `rg -n '0xfeed' src/core/mod.rs` to confirm exact lines first.
//
// CRITICAL (G2 — NO existing test breaks; don't hunt for one): exhaustive grep proves
//   no test asserts the literal "0xfeed" template STRING. The round-trip tests
//   (mod.rs:510 render_default_config_template_round_trips_to_defaults, mod.rs:524
//   render_config_body_round_trips) assert PARSED field VALUES (cfg.vendor_id == None)
//   and still pass. The other 0xfeed hits in src/ (tray.rs/linux.rs/notifier.rs tests,
//   parse_id, render_vidpid_rule) use 0xfeed as a config VALUE / fixture — UNRELATED,
//   leave them. ADD the §9 gate test; do NOT "fix" a non-existent template-text test.
//
// CRITICAL (G3 — path-keyed kind lookup, NOT vid/pid): match each enumerated device
//   to its classification by d.path().to_string_lossy().to_string() ==
//   ClassifiedDevice.path. enumerate_candidates (@992) builds Candidate.path the SAME
//   way. vid/pid+usage can COLLIDE (split keyboards share it); path is the unique
//   stable key across the two HidApi snapshots.
//
// GOTCHA (G4 — classify_devices is filter-scoped): it classifies only Tier-1 boards
//   matching configured_filter() (reads config.toml VID/PID). With VID/PID set, non-
//   matching boards show '-'; common case (no VID/PID) classifies all Tier-1. This is
//   correct + informative — document in the doc-comment, don't bypass the filter.
//
// GOTCHA (G5 — degrades cleanly): classify_devices returns Vec::new() on any HID
//   error (enumerate_candidates ⇒ Vec::new() on Err). Empty kind map ⇒ every cell
//   '-'. Never panic, never Err. The "no devices / no capable board" guard IS this
//   natural empty handling + unwrap_or("-"). No special-case branch needed.
//
// GOTCHA (G6 — verbose on stderr, table on stdout): classify_devices(verbose=true)
//   prints probe progress via eprintln! (stderr); the table is println! (stdout). So
//   `--list-devices` (no -v) ⇒ clean table; `-v --list-devices` ⇒ table on stdout +
//   probe diagnostics on stderr. Thread `verbose` into list_devices(verbose) and on
//   to classify_devices(verbose). Do NOT silence -v here.
//
// GOTCHA (G7 — single-threaded tests, AGENTS.md): cargo test --bin qmkonnect --
//   --test-threads=1 (shared MockNotifier globals + DebounceState). The 2 new tests
//   are pure; list_devices end-to-end is hardware-gated (manual, Level 4).
//
// CRITICAL (G8 — SCOPE WALL, no docs/spec this task): do NOT edit README.md,
//   docs/configuration.md, docs/llms_full.txt, spec/*.md, Cargo.toml, tray.rs, or
//   linux_tray.rs. The README/docs 0xfeed cleanup is P4.M1.T1.S2 + P4.M2.T1.S1.
//   Contract point 5: "[Mode A] The template comment IS the user-facing doc here."
//
// GOTCHA (G9 — binary-only crate; doctests): no lib.rs (src/main.rs declares mod core).
//   Mode-A rustdoc on list_devices: use ```rust,ignore or prose (don't add a runnable
//   `use qmkonnect::...` doctest — won't compile under --bin).
//
// GOTCHA (G10 — 0x???? is NOT valid TOML hex): the new template line is COMMENTED
//   (`# vendor_id  = 0x????...`), so it never reaches the parser. The round-trip test
//   still parses the template to all-None defaults. Do NOT make 0x???? an uncommented
//   value. (The Some arms stay real hex: `vendor_id = 0x{v:04x}`.)
```

## Implementation Blueprint

### Data models and structure

```rust
// ── (1) src/core/mod.rs — render_default_config_template (lines 238-239) ──
// BEFORE:
//      # vendor_id  = 0xfeed   # unset: auto-discovery\n\
//      # product_id = 0x0000   # unset: auto-discovery\n"
// AFTER (verbatim from spec/DEVICE_DISCOVERY.md §7.2):
//      # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n\
//      # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n"

// ── (2) src/core/mod.rs — render_config_body None arms (line 258 + ~261) ──
// BEFORE:
//      None => "# vendor_id  = 0xfeed   # unset: auto-discovery".to_string(),
//      ...
//      None => "# product_id = 0x0000   # unset: auto-discovery".to_string(),
// AFTER (identical text to (1), in BOTH None arms):
//      None => "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
//      ...
//      None => "# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
//   (the Some(v)/Some(p) arms are UNCHANGED: format!("vendor_id  = 0x{v:04x}") etc.)

// ── (3) src/core/notifier.rs — the kind-column label (pure; unit-tested) ──
/// The `--list-devices` kind-column label for a classified Tier-1 device
/// (`spec/DEVICE_DISCOVERY.md` §8): `qmk_notifier` for a capable board,
/// `qmk-only` for a QMK raw-HID board that isn't running the qmk_notifier module.
/// Pure; unit-tested (`kind_label_matches_spec`).
fn kind_label(kind: &DeviceKind) -> &'static str {
    match kind {
        DeviceKind::Capable { .. } => "qmk_notifier",
        DeviceKind::NotQmkNotifier => "qmk-only",
    }
}

// ── (4) src/core/notifier.rs — list_devices(verbose) with the kind column ──
/// Print every HID device the kernel can see, WITHOUT opening any of them — the
/// VID/PID discovery tool (`spec/DEVICE_DISCOVERY.md` §8 / `PROTOCOL.md` §6).
/// Read-only enumeration (never seizes a device). Adds a **`kind`** column from a
/// one-shot [`classify_devices`] pass: Tier-1 QMK raw-HID boards that answered the
/// capability probe show `qmk_notifier` (capable) or `qmk-only` (QMK board, no
/// qmk_notifier module); all other interfaces show `-`.
///
/// `classify_devices` runs against the *configured* filter, so when `vendor_id`/
/// `product_id` are set, boards outside that filter are not classified and show
/// `-` (the common no-VID/PID case classifies all Tier-1 boards). If the HID
/// classification itself fails, the kind map is empty and every cell is `-` — no
/// panic, no error (§7.2/§8).
///
/// `verbose` is forwarded to [`classify_devices`]: `-v` prints per-candidate probe
/// diagnostics to **stderr** (the **stdout** table stays clean).
pub fn list_devices(verbose: bool) -> Result<(), Box<dyn Error>> {
    let api = hidapi::HidApi::new()?;

    // One-shot Tier-2 classification (cache-backed; pings only on a cold/stale
    // cache). Keyed by the stable hidapi `path` (mirrors enumerate_candidates)
    // so each enumerated interface maps to its own classification. Returns [] on
    // any HID error ⇒ the kind column degrades to `-` everywhere (G5).
    let kind_by_path: std::collections::HashMap<String, DeviceKind> =
        classify_devices(verbose)
            .into_iter()
            .map(|c| (c.path, c.kind))
            .collect();

    println!("Available HID devices (vendor:product  usage_page:usage  product  kind):");
    for d in api.device_list() {
        let kind = kind_by_path
            .get(&d.path().to_string_lossy().to_string())
            .map(kind_label)
            .unwrap_or("-");
        println!(
            "  {:#06x}:{:#06x}  {:#06x}:{:#06x}  {}  {}",
            d.vendor_id(),
            d.product_id(),
            d.usage_page(),
            d.usage(),
            d.product_string().unwrap_or(""),
            kind,
        );
    }
    Ok(())
}
```

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: REPLACE the 0xfeed literals in render_default_config_template (src/core/mod.rs:238-239)
  - DO: change the two lines (verbatim BEFORE/AFTER in "Data models" block (1)):
        # vendor_id  = 0xfeed   # unset: auto-discovery   →   # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)
        # product_id = 0x0000   # unset: auto-discovery   →   # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)
    Mind the Rust string-literal continuation: the vendor line ends with `\n\` (a
    trailing backslash-newline inside the concat) and the product line ends with
    `\n"` (closing the literal). Preserve EXACT spacing (the `=  ` alignment +
    the `# ` comment prefix). Target text is verbatim from spec/DEVICE_DISCOVERY.md §7.2.
  - VERIFY: `rg -n '0xfeed' src/core/mod.rs` should now show NO hit in the template
    (only line 22's comment + the test fixtures at 529/572/696, which are VALUES).
  - GOTCHA G1: this is ONE of TWO renderers — Task 2 does the other.
  - GOTCHA G10: the new line stays COMMENTED (0x???? is not valid TOML hex).

Task 2: REPLACE the 0xfeed literals in render_config_body None arms (src/core/mod.rs:258 + ~261)
  - DO: change BOTH None arms (verbatim BEFORE/AFTER in "Data models" block (2)):
        None => "# vendor_id  = 0xfeed   # unset: auto-discovery".to_string(),
          →  None => "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
        None => "# product_id = 0x0000   # unset: auto-discovery".to_string(),
          →  None => "# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
    The Some(v)/Some(p) arms are UNCHANGED. Always grep first to confirm the pid
    None arm's exact line (~261, may drift ±1).
  - WHY (G1): render_config_body is the SAVE renderer — every Settings-dialog save
    calls it. Updating only the template (Task 1) would leave saves re-injecting 0xfeed.
  - VERIFY: `rg -n '0xfeed' src/core/mod.rs` now shows ZERO hits in lines 200-300
    (both renderers clean); only the comment @22 + test fixtures @529/572/696 remain.

Task 3: ADD kind_label + REWRITE list_devices(verbose) (src/core/notifier.rs:129)
  - DO (3a): add the kind_label pure helper (verbatim in "Data models" block (3)),
        placed just ABOVE list_devices (or alongside the classify_devices area).
  - DO (3b): replace list_devices()'s body + signature with list_devices(verbose: bool)
        (verbatim in "Data models" block (4)) — same enumerate loop, + the
        kind_by_path HashMap build + the kind column in the header + each row.
  - IMPORTS: std::collections::HashMap is used fully-qualified
        (std::collections::HashMap<...>) in the body — NO new `use` needed. If you
        prefer a `use`, add `use std::collections::HashMap;` near the top of the
        classify section. DeviceKind + classify_devices are in-scope (same module).
  - DOC-COMMENT (Mode A, G9): the `///` block in "Data models" (4) — cite §8 + §7.2,
        note path-keyed classify + the filter-scope caveat + stderr/stdout split.
  - GOTCHA G3: path-keyed (d.path().to_string_lossy().to_string()), NOT vid/pid.
  - GOTCHA G5: empty kind map ⇒ all '-', no panic (classify_devices ⇒ Vec::new() on Err).
  - GOTCHA G6: forward `verbose` to classify_devices(verbose); -v diagnostics on stderr.

Task 4: UPDATE the --list-devices dispatch (src/main.rs:~117)
  - DO: change
          crate::core::notifier::list_devices()?;
        to
          crate::core::notifier::list_devices(verbose)?;
    (verbose is captured near the top of the args fn and is in scope here — it's
    already used by list_callbacks(verbose)/validate_rules(...,verbose) below.)
  - VERIFY: `rg -n 'list_devices' src/` shows exactly 2 hits: the def @notifier.rs:129
    and the call @main.rs:117. No other call site.

Task 5: ADD the 2 unit tests
  - DO (5a, in src/core/mod.rs #[cfg(test)] mod tests, near the round-trip tests @510/@524):
        #[test]
        fn template_has_no_0xfeed_literal() {
            // §9 gate: "the seeded template contains no literal 0xfeed."
            let seeded = render_default_config_template();
            assert!(!seeded.contains("0xfeed"), "seeded template still has 0xfeed: {seeded:?}");
            assert!(seeded.contains("0x????"), "seeded template missing the 0x???? hint: {seeded:?}");
            // The save renderer's None body (Config::default() = all None) must ALSO
            // be clean — G1: both renderers.
            let saved = render_config_body(&Config::default());
            assert!(!saved.contains("0xfeed"), "save-renderer None body still has 0xfeed: {saved:?}");
            assert!(saved.contains("0x????"), "save-renderer None body missing 0x????: {saved:?}");
        }
  - DO (5b, in src/core/notifier.rs #[cfg(test)] mod tests, alongside the classify tests):
        #[test]
        fn kind_label_matches_spec() {
            use super::{kind_label, DeviceKind};
            let capable = DeviceKind::Capable { proto_ver: 2, feature_flags: 1,
                callback_count: 0, board_rules_present: false };
            assert_eq!(kind_label(&capable), "qmk_notifier");
            assert_eq!(kind_label(&DeviceKind::NotQmkNotifier), "qmk-only");
        }
  - GOTCHA G2: do NOT modify the existing round-trip tests — they still pass as-is.

Task 6: VERIFY build + tests (single-threaded, per AGENTS.md)
  - RUN: cargo build --bin qmkonnect          (expect clean; no NEW warnings)
  - RUN: cargo test --bin qmkonnect -- --test-threads=1
         (expect: ALL green — existing tests unchanged + template_has_no_0xfeed_literal
          + kind_label_matches_spec)
  - CONFIRM git status shows EXACTLY 3 files: src/core/mod.rs, src/core/notifier.rs,
    src/main.rs (G8: NOTHING in README.md, docs/*, spec/*, Cargo.toml, tray.rs, linux_tray.rs).
```

### Implementation Patterns & Key Details

```rust
// The 0xfeed → 0x???? replacement is a verbatim text swap (spec §7.2). The ONLY
// subtlety is doing it in BOTH renderers (G1) and keeping the line COMMENTED (G10):
//   render_default_config_template: "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)"
//   render_config_body None arm:     "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)"
// (identical text; the Some arms stay `vendor_id = 0x{v:04x}` — real hex.)

// The kind column is a path-keyed join (G3) of the all-devices enumerate onto the
// Tier-1 classification:
//   let kind_by_path: HashMap<String, DeviceKind> =
//       classify_devices(verbose).into_iter().map(|c| (c.path, c.kind)).collect();
//   ... for d in api.device_list() {
//       let kind = kind_by_path.get(&d.path().to_string_lossy().to_string())
//           .map(kind_label).unwrap_or("-");
//   }
// enumerate_candidates (@992) builds ClassifiedDevice.path the SAME way (d.path()
// .to_string_lossy().to_string()), so the two HidApi snapshots join correctly.

// list_devices signature change → main.rs dispatch (the ONLY call site):
//   src/main.rs:117:  crate::core::notifier::list_devices(verbose)?;
// (verbose is in scope; the -v diagnostics from classify_devices go to stderr, the
//  table to stdout — G6.)
```

### Integration Points

```yaml
CODE (this task):
  - file: src/core/mod.rs
    change: "4 literal replacements (0xfeed/0x0000 → 0x???? + §7.2 comment) in BOTH
             render_default_config_template (238-239) and render_config_body's None
             arms (258 + ~261); + template_has_no_0xfeed_literal test."
  - file: src/core/notifier.rs
    change: "list_devices() → list_devices(verbose: bool) with a path-keyed kind
             column + kind_label() pure helper + Mode-A doc-comment + kind_label_matches_spec
             test. classify_devices/DeviceKind/ClassifiedDevice CONSUMED unchanged."
  - file: src/main.rs
    change: "--list-devices dispatch: list_devices()? → list_devices(verbose)? (1 line)."

DEPENDENCIES (this task): NONE new. std::collections::HashMap (used fully-qualified
                           or via a `use`). No Cargo change. classify_devices +
                           DeviceKind + ClassifiedDevice are pub + in-tree (P3.M1).

UPSTREAM (consumed read-only):
  - classify_devices(verbose: bool) -> Vec<ClassifiedDevice> (notifier.rs:1116).
  - DeviceKind { Capable{..}, NotQmkNotifier } (notifier.rs:818; Clone).
  - ClassifiedDevice { path:String, vendor_id:u16, product_id:u16, kind:DeviceKind, .. } (notifier.rs:843).
  - hidapi::DeviceInfo::path() (the join key; same value enumerate_candidates stringifies).
  - Config { vendor_id:Option<u16>, product_id:Option<u16>, .. } (mod.rs:23) — UNCHANGED.

DOWNSTREAM / SIBLINGS (do NOT implement them here):
  - P4.M1.T1.S2 (Mode-A user-facing doc sync): mirrors the 0x???? wording into README/docs.
  - P4.M2.T1.S1 (README audit + regenerate llms_full.txt): the README/docs 0xfeed cleanup.
  - Neither is blocked by this task (the code renderers are independent of the docs).

CONFIG: none (Config schema unchanged — vendor_id/product_id stay Option<u16>).
ROUTES: none (no new CLI flag — --list-devices already exists). DATABASE: none.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# Expected: compiles clean, no NEW warnings. If it fails: most likely a typo in the
# template string-literal continuation (the trailing `\` + alignment spaces) OR a
# main.rs call mismatch (list_devices(verbose)? vs the new signature) — READ + fix.

# Confirm BOTH renderers are clean and the kind column is present:
rg -n '0xfeed|0x0000' src/core/mod.rs | rg -v '://|test|0xfeed\\n|assert|Some\(0xfeed\)|"vendor_id = 0xfeed'
#   (expect: NO hits in the 238-261 range — both renderers use 0x???? now. The
#    remaining 0xfeed hits are the comment @22 + the test fixtures @529/572/696,
#    which are config VALUES, not template literals.)
rg -n '0x\?\?\?\?' src/core/mod.rs          # expect: 4 hits (2 template + 2 None arms)
rg -n 'fn list_devices' src/core/notifier.rs  # expect: pub fn list_devices(verbose: bool)
rg -n 'fn kind_label' src/core/notifier.rs    # expect: 1
rg -n 'list_devices\(verbose\)' src/main.rs   # expect: 1 (the dispatch)
rg -n 'kind' src/core/notifier.rs | rg 'println|header'   # expect: the header + row kind field
```

### Level 2: Unit Tests (the pure gates)

```bash
cd /home/dustin/projects/qmkonnect
# Single-threaded is MANDATORY crate-wide (shared MockNotifier globals + DebounceState, AGENTS.md / G7).
cargo test --bin qmkonnect -- --test-threads=1
# Expected: ALL green — the existing tests (incl. the round-trip tests @510/@524,
# which assert parsed field VALUES and still pass) + template_has_no_0xfeed_literal
# + kind_label_matches_spec. The list_devices end-to-end path is NOT unit-testable
# (real HID) — covered by Level 4.

cargo test --bin qmkonnect template_has_no_0xfeed_literal -- --test-threads=1
cargo test --bin qmkonnect kind_label_matches_spec        -- --test-threads=1
# Expected: both pass (the §9 gate + the kind-label contract).
```

### Level 3: Regression (the 0xfeed cleanup doesn't break parsing)

```bash
cd /home/dustin/projects/qmkonnect
# The round-trip tests are the proof that the commented 0x???? line still parses to
# all-None defaults (the cleanup doesn't change Config semantics):
cargo test --bin qmkonnect render_default_config_template_round_trips -- --test-threads=1
cargo test --bin qmkonnect render_config_body_round_trips            -- --test-threads=1
# Expected: both pass (the template + the None body still parse to vendor_id=None etc.).

# Confirm the change surface is exactly the 3 source files (G8):
git status --short
# Expected: src/core/mod.rs, src/core/notifier.rs, src/main.rs ONLY.
# NOTHING in README.md, docs/*, spec/*, Cargo.toml, tray.rs, linux_tray.rs, platforms/*.
git diff --stat
# Expected: 3 files.
```

### Level 4: Manual `--list-devices` validation (hardware-gated)

```bash
cd /home/dustin/projects/qmkonnect
cargo build --bin qmkonnect
# 1. With NO QMK board:  ./target/debug/qmkonnect --list-devices
#    Expected: the header "...product  kind):" prints; rows for whatever HID is
#    present all show "-" in the kind column (no panic, clean table).
# 2. With a qmk_notifier-capable board: the board's row shows "qmk_notifier".
# 3. With a VIA/Vial-only QMK board (raw-HID 0xFF60/0x61, no module): shows "qmk-only".
# 4. With -v:  ./target/debug/qmkonnect -v --list-devices
#    Expected: the same stdout table PLUS classify_devices per-candidate probe
#    diagnostics on stderr (table on stdout stays clean — pipe separately to verify).
# 5. The seeded config (qmkonnect -c then open the new config.toml): the file shows
#    "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)"
#    — NO literal 0xfeed.
# 6. Open Settings, save (pick or manual) → reopen config.toml: the None/blank fields
#    render as the commented "0x????" line (the save renderer is clean too — G1).
```

## Final Validation Checklist

### Technical Validation
- [ ] `cargo build --bin qmkonnect` clean (no NEW warnings).
- [ ] `cargo test --bin qmkonnect -- --test-threads=1` green (existing + 2 new).
- [ ] `git diff --stat` = `src/core/mod.rs` + `src/core/notifier.rs` + `src/main.rs` only.

### Feature Validation (contract fidelity)
- [ ] **`render_default_config_template`** (mod.rs:238-239): no `0xfeed`, has `0x????`,
      comment is the §7.2 text; still parses to all-None defaults.
- [ ] **`render_config_body` `None` arms** (mod.rs:258 + ~261): no `0xfeed`, has
      `0x????`, §7.2 comment; `Some` arms UNCHANGED (G1 — both renderers).
- [ ] **`list_devices(verbose: bool)`**: path-keyed `kind` column; `Capable`⇒
      `qmk_notifier`, `NotQmkNotifier`⇒`qmk-only`, unmatched⇒`-`; empty/degraded⇒all
      `-`, no panic (G3/G5).
- [ ] **`kind_label`** pure helper returns the §8 labels.
- [ ] **main.rs** dispatch threads `verbose` (G6: -v diagnostics on stderr).
- [ ] **Mode-A doc-comment** on `list_devices` cites §8 + §7.2.

### Code Quality Validation
- [ ] `template_has_no_0xfeed_literal` (§9 gate) + `kind_label_matches_spec` added.
- [ ] Existing round-trip tests + all other tests UNCHANGED + still pass (G2).
- [ ] No new Cargo deps; Config schema unchanged; no new CLI flag.
- [ ] Follows existing renderer patterns (pure String builders; commented hints).

### Documentation & Deployment
- [ ] Mode-A rustdoc on `list_devices`; the template comment IS the user-facing doc
      (contract point 5) — NO docs/*.md / README edit this task (G8).
- [ ] Commit message notes the cleanup is code-only; the README/docs mirror is
      P4.M1.T1.S2 / P4.M2.T1.S1.

---

## Anti-Patterns to Avoid

- ❌ Do NOT update only `render_default_config_template`. `render_config_body`'s
      `None` arms emit the SAME `0xfeed` literal and are the SAVE renderer — every
      Settings save calls them. Update BOTH (G1).
- ❌ Do NOT hunt for an existing test to "fix" — none asserts the literal `0xfeed`
      template string. The round-trip tests assert parsed field values and still
      pass. ADD the §9 gate test instead (G2).
- ❌ Do NOT key the kind column by (vid,pid,usage_page,usage). Split keyboards share
      that tuple. Use the stable hidapi `path` (G3) — the same key
      `enumerate_candidates` uses.
- ❌ Do NOT bypass `configured_filter` in the kind column. `classify_devices` is
      filter-scoped by design; non-matching boards show `-` (correct + informative).
      Document it; don't "fix" it (G4).
- ❌ Do NOT panic/`Err` when `classify_devices` returns empty. The empty kind map ⇒
      all `-` cells. The "no devices / no capable board" guard IS this natural
      empty handling (G5).
- ❌ Do NOT make `0x????` an uncommented value — it's not valid TOML hex. The line
      stays COMMENTED (`# vendor_id = 0x????...`); the parser never sees it (G10).
- ❌ Do NOT touch `README.md`, `docs/*.md`, `docs/llms_full.txt`, `spec/*.md`,
      `Cargo.toml`, `tray.rs`, or `linux_tray.rs`. The doc-literal cleanup is
      P4.M1.T1.S2 + P4.M2.T1.S1 (G8 scope wall; contract point 5).
- ❌ Do NOT silence `-v` for `--list-devices`. Thread `verbose` into
      `list_devices(verbose)` and on to `classify_devices(verbose)` so the probe
      diagnostics reach stderr while the table stays clean on stdout (G6).
- ❌ Do NOT add runnable Rust doctests (` ``` `) that `use qmkonnect::...`. Binary-only
      crate (no lib.rs); use ` ```rust,ignore ` or prose (G9).
- ❌ Do NOT run tests multi-threaded — `cargo test --bin qmkonnect --
      --test-threads=1` (shared globals, G7/AGENTS.md).
- ❌ Do NOT edit `PRD.md`, any `tasks.json`, `prd_snapshot.md`, any `spec/*.md`, or
      any `plan/` file other than this item's own `PRP.md` + `research/`.

---

## Confidence Score: 9/10

Two mechanical, verbatim text replacements (target text pinned from spec §7.2) +
a small, well-bounded `list_devices` extension that consumes an already-Complete,
already-tested `classify_devices` API whose exact signatures and path-keying are
verified in-tree. The exhaustive-grep proof that no existing test breaks (G2) and
the CRITICAL both-renderers note (G1) are the two implementation traps, both
explicitly pinned. Scope is code-only with a clean wall to the doc-sync siblings
(G8). The only un-automatable piece (`list_devices` end-to-end) is hardware-gated
manual validation (Level 4).