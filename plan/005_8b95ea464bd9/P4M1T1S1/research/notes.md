# Research Notes — P4.M1.T1.S1: Config template `0xFEED` cleanup + `--list-devices` kind column

> **Repo under change:** the **qmkonnect** desktop app (Rust) at
> `/home/dustin/projects/qmkonnect`. This task is **CODE-only** across two files:
> (1) `src/core/mod.rs` — replace the `0xfeed`/`0x0000` literals in BOTH config
> renderers (`render_default_config_template` + `render_config_body`'s `None`
> arms) with `0x????` + the spec §7.2 comment; (2) `src/core/notifier.rs` — give
> `list_devices()` a `kind` column sourced from a one-shot `classify_devices()`
> pass; plus (3) `src/main.rs` — thread `verbose` into the (now-signature-changed)
> `list_devices(verbose)` dispatch. **Consumes** `classify_devices(verbose) ->
> Vec<ClassifiedDevice>` + `DeviceKind`/`ClassifiedDevice` (P3.M1.T1.S2 — Complete).
>
> **SCOPE WALL (critical):** this task does NOT touch `README.md`,
> `docs/configuration.md`, `docs/llms_full.txt`, or any `spec/*.md`. The
> `0xfeed` literals that ALSO appear in README/docs (config_cli.md table) are
> owned by the **doc-sync** siblings **P4.M1.T1.S2** (Mode-A user-facing doc
> sync) and **P4.M2.T1.S1** (README audit + regenerate llms_full.txt). Contract
> point 5: "[Mode A] The template comment IS the user-facing doc here; no
> separate docs/*.md edit for the literal."
>
> **PARALLEL-EXECUTION NOTE:** P3.M2.T1.S3 (Linux zenity picker) is being
> implemented concurrently and touches `src/linux_tray.rs` ONLY. This task
> touches `src/core/mod.rs` + `src/core/notifier.rs` + `src/main.rs` — fully
> disjoint files; no merge conflict and no shared symbol.

---

## 0. The two config renderers (the `0xfeed` cleanup) — `src/core/mod.rs`

### 0.1 `render_default_config_template` (the `-c` seeder / first-run template)

Verbatim current tail (grep-confirmed: the `0xfeed` literal is at **line 238**,
`0x0000` at **line 239**):

```rust
pub fn render_default_config_template() -> String {
    "# QMKonnect Configuration\n\
     ...
     # vendor_id  = 0xfeed   # unset: auto-discovery\n\        // ← line 238
     # product_id = 0x0000   # unset: auto-discovery\n"        // ← line 239
        .to_string()
}
```

**Target (spec/DEVICE_DISCOVERY.md §7.2, lines 380-381 — verbatim):**
```rust
     # vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n\
     # product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)\n"
```

### 0.2 `render_config_body` `None` match arms (the SAVE renderer — the contract's CRITICAL note)

Verbatim current (grep-confirmed: vendor `None` arm at **line 258**, pid `None`
arm at **~line 261**; always grep to confirm before editing):

```rust
pub fn render_config_body(config: &Config) -> String {
    let vid_line = match config.vendor_id {
        Some(v) => format!("vendor_id  = 0x{v:04x}"),
        None => "# vendor_id  = 0xfeed   # unset: auto-discovery".to_string(),   // ← line 258
    };
    let pid_line = match config.product_id {
        Some(p) => format!("product_id = 0x{p:04x}"),
        None => "# product_id = 0x0000   # unset: auto-discovery".to_string(),   // ← ~line 261
    };
    ...
}
```

**Target (identical text to §0.1's, in both `None` arms):**
```rust
        None => "# vendor_id  = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
        ...
        None => "# product_id = 0x????   # unset: auto-discover any QMK keyboard (recommended)".to_string(),
```

> **CRITICAL (the contract's headline gotcha):** the PRD §7.2 only NAMES
> `render_default_config_template`, but `render_config_body` ALSO emits the same
> `0xfeed`/`0x0000` literals in its `None` arms. **BOTH renderers must be
> updated** or the Settings-dialog save path (which calls `render_config_body`)
> would keep writing the old literal into the user's `config.toml` on every save
> — defeating the cleanup. config_cli.md's "0xFEED Locations" table confirms both
> code sites (lines 238-239 + 258/~261).

---

## 1. Existing tests — NONE assert the literal `0xfeed` template text

`rg -n '0xfeed' src/core/mod.rs` returns these test sites (all use `0xfeed` as a
**config VALUE / parse INPUT**, not a rendered-template string assertion):

| Line | Test / site | Role of `0xfeed` | Update? |
|------|-------------|------------------|---------|
| 22 | module comment | prose ("keep working") | NO (still true) |
| 238-239 | `render_default_config_template` body | the literal being REMOVED | YES (this task) |
| 258, ~261 | `render_config_body` None arms | the literal being REMOVED | YES (this task) |
| 529-530 | `legacy_config_with_explicit_ids_parses_to_some` | parse INPUT `"vendor_id = 0xfeed"` → `Some(0xfeed)` | **NO** — 0xfeed stays a valid legacy config value |
| 572, 577 | `render_config_body_round_trips` | `Some(0xfeed)` as Config INPUT (tests the `Some` arm) | **NO** — `Some` arm unchanged |
| 696, 700 | `test_atomic_write_*` | `"vendor_id = 0xfeed"` as atomic_write INPUT | **NO** — just test fixture content |

**The two renderer round-trip tests** (`render_default_config_template_round_trips_to_defaults`
@510, `render_config_body_round_trips` @524) assert **parsed field values**
(`cfg.vendor_id == None`), NOT literal template strings. After this task:
- the template still parses to all-`None` defaults (the `0x????` line is commented
  out AND not valid hex anyway) ⇒ the round-trip test still passes.
- the `render_config_body` `None` arm still renders a COMMENTED line ⇒ parses to
  `None` ⇒ still passes. The `Some(0xfeed)` arm is UNCHANGED ⇒ still passes.

**∴ No existing test breaks.** The contract's "Update any test asserting the old
template text" resolves to: **there are none to update.** Instead, ADD the §9
gate test ("the seeded template contains no literal `0xfeed`") as the positive
assertion of the cleanup — see §4/test plan.

> The `0xfeed` occurrences in `src/tray.rs` (3015/3042/3057/3084),
> `src/linux_tray.rs` (1055-1056), `src/platforms/linux.rs` (444-503),
> `src/core/notifier.rs` (2319/3490) are all **unrelated test fixtures / config
> values / ClassifiedDevice fixtures** — NOT template-text assertions. Leave them.

---

## 2. `list_devices()` + the kind column — `src/core/notifier.rs`

### 2.1 Current `list_devices()` (grep-confirmed @ line 129)

```rust
pub fn list_devices() -> Result<(), Box<dyn Error>> {
    let api = hidapi::HidApi::new()?;
    println!("Available HID devices (vendor:product  usage_page:usage  product):");
    for d in api.device_list() {
        println!(
            "  {:#06x}:{:#06x}  {:#06x}:{:#06x}  {}",
            d.vendor_id(), d.product_id(), d.usage_page(), d.usage(),
            d.product_string().unwrap_or(""),
        );
    }
    Ok(())
}
```
- Enumerates **ALL** hidapi interfaces (not just Tier-1) — it's the VID/PID
  discovery tool (PRD §6 / h2.29). Read-only; never opens a device. **THIS TASK
  ADDS** a `kind` column + threads `verbose`.
- **Only call site:** `src/main.rs:117` (`crate::core::notifier::list_devices()?;`).
  No other caller, no existing test for it. Changing the signature to
  `list_devices(verbose: bool)` is a safe 1-site update.

### 2.2 `classify_devices` (consumed; P3.M1.T1.S2 — Complete) — the kind source

```rust
pub fn classify_devices(verbose: bool) -> Vec<ClassifiedDevice>   // notifier.rs:1116
// body: enumerate_candidates() (Tier-1 only, via configured_filter) →
//       invalidate_absent_cache_entries → classify_candidates (per-candidate
//       QUERY_INFO ping, 5s-TTL cache). Returns Vec<ClassifiedDevice>.
```
```rust
pub enum DeviceKind {                                            // notifier.rs:818
    Capable { proto_ver: u8, feature_flags: u8, callback_count: u8, board_rules_present: bool },
    NotQmkNotifier,
}   // #[derive(Debug, Clone, PartialEq)]

pub struct ClassifiedDevice {                                    // notifier.rs:843
    pub path: String,            // = d.path().to_string_lossy().to_string() (see enumerate_candidates @992)
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_name: Option<String>,
    pub usage_page: u16,
    pub usage: u16,
    pub kind: DeviceKind,
}   // #[derive(Debug, Clone, PartialEq)]
```

**Key matching fact:** `enumerate_candidates()` (notifier.rs:992) builds each
`Candidate.path` as `d.path().to_string_lossy().to_string()` from the SAME hidapi
`DeviceInfo::path()` that `list_devices()` iterates. So the kind-column lookup is
a **path-keyed `HashMap<String, DeviceKind>`**: build it from
`classify_devices(verbose)`, then for each enumerated `d` look up
`d.path().to_string_lossy().to_string()`. Path strings are stable across the two
`HidApi::new()` snapshots (they're OS device paths).

**Filter caveat (document, Mode A):** `classify_devices` runs against
`configured_filter()` (reads `config.toml`'s VID/PID). If the user has narrowed
VID/PID, only matching Tier-1 boards are classified; the rest show `-` in the
kind column. The common `--list-devices` case (no VID/PID set ⇒ match-any)
classifies **all** Tier-1 boards. This is correct and informative.

**Degradation:** `classify_devices` returns `Vec::new()` on any HID error
(`enumerate_candidates` ⇒ `Vec::new()` on `Err`) — so the kind map is just empty
⇒ every kind cell shows `-`. **No panic, no error.** This satisfies the
contract's "Guard so `--list-devices` against no devices / no capable board still
prints cleanly."

### 2.3 The kind labels (spec §8 + contract)

spec/DEVICE_DISCOVERY.md §8 CLI row: "`--list-devices` output gains a `kind`
column (`qmk_notifier` / `qmk-only` / `via-only`-ish, from a one-shot
`classify_devices`)". Contract: `'qmk_notifier'` for `Capable`, `'qmk-only'` for
`NotQmkNotifier`. Non-Tier-1 / unmatched interfaces (mice, etc., not in
`classify_devices` output) ⇒ `-` (short placeholder; keeps the column readable).
**Centralize in a tiny pure helper `kind_label(&DeviceKind) -> &'static str`** so
the §8 wording lives in one unit-tested place.

### 2.4 `--list-devices` dispatch — `src/main.rs:115-119`

```rust
if args.iter().any(|arg| arg == "--list-devices") {
    crate::core::notifier::list_devices()?;          // ← becomes list_devices(verbose)?
    return Ok(());
}
```
- `verbose` is captured once near the top of the args-processing fn (used later by
  `list_callbacks(verbose)`, `validate_rules(..., verbose)` ⇒ in scope here).
- **Change:** `list_devices(verbose)?`. Threading `verbose` lets
  `qmkonnect -v --list-devices` surface `classify_devices`'s per-candidate probe
  diagnostics (to **stderr** via `eprintln!`, so the **stdout** table stays clean)
  — useful when debugging "why isn't my board detected". Without `-v`,
  `classify_devices(false)` is silent ⇒ a clean table. (§8 lists `src/main.rs` as
  a touched file — this 1-line dispatch update is that touch.)

---

## 3. Design decisions (RESOLVED)

- **D1 — BOTH renderers updated, identical target text.** The contract's CRITICAL
  note: `render_config_body` (the save renderer) emits the same literal in its
  `None` arms; updating only `render_default_config_template` would leave the
  Settings-save path re-injecting `0xfeed` on every save. Both get the spec §7.2
  text (`0x????` + "unset: auto-discover any QMK keyboard (recommended)").
- **D2 — `list_devices(verbose: bool)` signature change + main.rs dispatch update.**
  Threads `verbose` to `classify_devices(verbose)` so `-v` shows probe diagnostics
  (stderr; stdout table stays clean). Matches §8's file list (notifier.rs +
  main.rs). Safe: 1 call site (main.rs:117), no other caller, no existing test.
  (Minimal-risk alternative: keep `list_devices()` zero-arg + `classify_devices(false)`
  internal — but then `-v --list-devices` can't surface probe diagnostics and
  §8's main.rs touch is unmet. Prefer the threaded version.)
- **D3 — kind column via path-keyed `HashMap`, defensive.** Build
  `HashMap<String, DeviceKind>` from `classify_devices(verbose)`; look up each
  enumerated `d` by `d.path().to_string_lossy().to_string()`. `Capable` ⇒
  `"qmk_notifier"`, `NotQmkNotifier` ⇒ `"qmk-only"`, unmatched ⇒ `"-"`.
  `classify_devices` returning `[]` (HID error / no Tier-1) ⇒ all cells `"-"`,
  no panic. Satisfies the "prints cleanly on no devices / no capable board" guard.
- **D4 — no existing test breaks; ADD the §9 gate test + a kind_label test.** The
  round-trip tests assert parsed field values (not literal strings) and still
  pass. Add (a) `template_has_no_0xfeed_literal` (mod.rs): the rendered template
  AND `render_config_body(&Config::default())` contain no `0xfeed` and DO contain
  `0x????` (the §9 gate: "the seeded template contains no literal `0xfeed`"); and
  (b) `kind_label_matches_spec` (notifier.rs): `Capable{..}` ⇒ `"qmk_notifier"`,
  `NotQmkNotifier` ⇒ `"qmk-only"`. `list_devices` end-to-end is manual-only
  (real HID) — Level 4.
- **D5 — Mode-A doc IS the template comment; no docs/*.md edit this task.**
  Contract point 5. The `0xfeed` in README.md / docs/configuration.md /
  llms_full.txt is owned by P4.M1.T1.S2 + P4.M2.T1.S1 (doc sync). Add a Mode-A
  doc-comment on `list_devices` citing spec/DEVICE_DISCOVERY.md §8 + §7.2.

---

## 4. Gotchas (pinned for the PRP)

- **G1 (BOTH renderers):** update `render_default_config_template` (lines 238-239)
  AND `render_config_body`'s `None` arms (line 258 + ~261). Missing the save
  renderer ⇒ Settings re-injects `0xfeed` on every save. grep to confirm exact
  lines (`rg -n '0xfeed' src/core/mod.rs`).
- **G2 (no test breaks; don't hunt for one):** no existing test asserts the
  literal `0xfeed` template string. The round-trip tests (510, 524) assert parsed
  field VALUES and still pass. ADD the §9 gate test; don't waste time "fixing" a
  non-existent template-text assertion.
- **G3 (path-keyed kind lookup, NOT vid/pid):** match enumerated devices to their
  classification by `d.path().to_string_lossy().to_string()` ==
  `ClassifiedDevice.path`. vid/pid+usage can collide (split keyboards share it);
  path is the unique stable key (mirrors `enumerate_candidates` @992).
- **G4 (classify_devices is filter-scoped):** it only classifies Tier-1 boards
  matching `configured_filter()`. With VID/PID set, non-matching boards show `-`.
  Common case (no VID/PID) classifies all Tier-1. Document; don't "fix" by
  bypassing the filter.
- **G5 (degrades cleanly):** `classify_devices` ⇒ `Vec::new()` on HID error;
  empty kind map ⇒ all cells `-`. Never panic, never `Err`. The "no devices / no
  capable board" guard is the natural empty handling + `unwrap_or("-")`.
- **G6 (verbose on stderr, table on stdout):** `classify_devices(verbose=true)`
  prints probe progress via `eprintln!` (stderr). The table is `println!` (stdout).
  So `--list-devices` (no -v) ⇒ clean table; `-v --list-devices` ⇒ table on stdout
  + probe diagnostics on stderr (pipeable separately). Thread `verbose`.
- **G7 (single-threaded tests, AGENTS.md):** `cargo test --bin qmkonnect --
  --test-threads=1`. The new tests (`template_has_no_0xfeed_literal`,
  `kind_label_matches_spec`) are pure; `list_devices` is hardware-gated (manual).
- **G8 (SCOPE WALL — no docs/spec this task):** do NOT edit README.md,
  docs/configuration.md, docs/llms_full.txt, spec/*.md, Cargo.toml, tray.rs, or
  linux_tray.rs. The doc-literal cleanup is P4.M1.T1.S2 / P4.M2.T1.S1.
- **G9 (binary-only crate; doctests):** no lib.rs. Mode-A rustdoc uses
  ` ```rust,ignore ` or prose (don't add runnable `use qmkonnect::...` doctests).
- **G10 (`0x????` is NOT valid TOML hex):** the new template line is COMMENTED
  (`# vendor_id  = 0x????...`), so it never reaches the TOML parser. The
  round-trip test still parses the template to all-`None` defaults. Do NOT make
  `0x????` an uncommented value.

---

## 5. Confidence

**9/10.** Two mechanical text replacements (verbatim target from spec §7.2) + a
small, well-bounded `list_devices` extension that consumes an already-Complete,
already-tested `classify_devices` API whose exact signatures + path-keying I
verified in-tree. The only residual risk (forgetting the save-renderer `None`
arms, G1) is pinned as the CRITICAL gotcha; the "no test breaks" fact (G2) is
verified by exhaustive grep. Scope is code-only with a clean wall to the doc-sync
siblings.