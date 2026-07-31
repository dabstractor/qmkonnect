//! Host-side `rules.toml` data model (PRD §9 / `spec/HOST_RULES.md` §9).
//!
//! These structs are the serde-deserialization boundary for `rules.toml`. They
//! are the typed, nested in-memory model that the resolver (effective
//! `disable_firmware_config` resolution — P3.M1.T1.S2) and the evaluator
//! (`evaluate()` — P3.M1.T2.S1) consume without re-parsing.
//!
//! This module defines the **model only**: it does not implement `parse_rules()`,
//! effective-resolution, or `evaluate()` (those are later subtasks — G7).
//!
//! # Schema source of truth
//!
//! Every derive, attribute, field, default, and rename here is verbatim from
//! `spec/HOST_RULES.md` §9. [`Pattern`] is imported (not redefined) from
//! [`crate::core::pattern`] (P2.M1.T3.S2).

// These structs are the data model for the host-rules system. They are exercised
// by the deserialization tests below and will be consumed cross-module by
// P3.M1.T1.S2 (`parse_rules` + effective resolution) and P3.M1.T2.S1
// (`evaluate`). Until those land, non-test builds flag them as unused —
// allow that here rather than at each call site (same idiom as pattern.rs:15).
#![allow(dead_code)]

use std::collections::{BTreeSet, HashMap};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::core::pattern::{match_pattern, Pattern}; // P2.M1.T3.S2 — Single/Parts, #[serde(untagged)]
use serde::Deserialize;

/// The top-level `rules.toml` model — a `[host]` defaults table plus ONE
/// table-array (`[[rule]]`, singular).
///
/// Every field is `#[serde(default)]`, so a partial/empty `rules.toml` parses to
/// an all-default ruleset (host stack default, empty rule vector). See
/// `spec/HOST_RULES.md` §9.
///
/// Evaluation is ONE pass over `[[rule]]` in file order (spec §8(3)): `layer`
/// is first-match-wins (one host layer active — exclusive); `enable`/`disable`
/// accumulate across ALL matches (all-match). A rule may set `layer` only,
/// callbacks only, or both.
///
/// ```toml
/// [host]
/// disable_firmware_config = false   # global default: false = stack (board runs), true = replace
/// # On no match the host layer is always cleared and all host callbacks disabled.
///
/// # Rules: one [[rule]] per (app × behavior). `layer` is first-match-wins
/// # (one host layer active — exclusive); `enable`/`disable` accumulate across
/// # ALL matches (all-match). A rule MUST set at least one of layer/enable/disable.
/// # `layer` is a raw QMK layer index (no reserved range; != 255) — see §3 C11.
/// [[rule]]
/// match = "alacritty"                       # class-only pattern
/// layer = 10
/// disable_firmware_config = true           # optional override (default inherits [host])
///
/// [[rule]]
/// match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
/// layer = 11
/// case_sensitive = false                    # optional, default false
///
/// [[rule]]
/// match = "neovide"
/// enable = ["vim_lazy", "disable_vim"]      # run on focus-in
/// disable = ["vim_lazy"]                    # optional: force-off override
///
/// [[rule]]
/// match = ["*chrome*", "*claude*"]
/// enable = ["vim_lazy", "disable_vim"]
/// disable_firmware_config = true           # for this window, skip the string -> board can't match
/// ```
#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    /// Global host defaults applied to every rule that does not override them.
    #[serde(default)]
    pub host: HostDefaults,
    /// Ordered rules. Evaluation is ONE pass in file order (spec §8(3)): `layer`
    /// is first-match-wins (one host layer — exclusive); `enable`/`disable`
    /// accumulate across ALL matches (all-match). TOML key is `[[rule]]`
    /// (SINGULAR — serde `rename = "rule"`).
    #[serde(default, rename = "rule")]
    pub rules: Vec<Rule>,
}

/// The `[host]` table — global defaults applied to every rule that does not
/// explicitly override them.
///
/// `disable_firmware_config` defaults to `false` (the board runs its own config:
/// the "stack" default). The derived `Default` makes that intent explicit
/// rather than relying on `bool`'s own default (see `spec/HOST_RULES.md` §9).
///
/// ```toml
/// [host]
/// disable_firmware_config = false   # global default: false = stack (board runs), true = replace
/// ```
#[derive(Debug, Default, Deserialize)]
pub struct HostDefaults {
    /// Global default for whether the board runs its own config (`false` = stack)
    /// or is replaced by the host layer (`true`). Per-rule
    /// `disable_firmware_config: Option<bool>` (on [`Rule`])
    /// overrides this; `None` inherits this value.
    #[serde(default)]
    pub disable_firmware_config: bool, // default false (stack)
}

/// A `[[rule]]` entry — maps a window [`Pattern`] to an optional host layer
/// and/or a set of callback names to `enable`/`disable` (by registry name).
///
/// The unified model (spec §9): one rule may set `layer` only, callbacks only,
/// or both. `match` (TOML key) is **required**; every other field is optional
/// with a `#[serde(default)]`. A rule that sets NONE of `layer`/`enable`/
/// `disable` is rejected at parse time by [`validate_rules`] (see spec §9
/// Validity).
///
/// Evaluation (spec §8(3), one pass): `layer` is **first-match-wins** among
/// layer-setting rules (one host layer — exclusive); `enable`/`disable` fire
/// **all-match** (every matching rule accumulates). `layer` is a **raw QMK
/// layer index** with no reserved range (only `255`/`0xFF` is rejected as the
/// wire "clear" sentinel — see `spec/HOST_RULES.md` §3 C11).
///
/// ```toml
/// [[rule]]
/// match = "alacritty"                       # class-only pattern (Pattern::Single)
/// layer = 10
/// disable_firmware_config = true           # optional override (default inherits [host])
///
/// [[rule]]
/// match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT(), Pattern::Parts)
/// layer = 11
/// case_sensitive = false                    # optional, default false
///
/// [[rule]]
/// match = "neovide"
/// enable = ["vim_lazy", "disable_vim"]      # run on focus-in
/// disable = ["vim_lazy"]                    # optional: force-off override
/// ```
#[derive(Debug, Deserialize)]
pub struct Rule {
    /// Window pattern (TOML key `match`). A bare string → [`Pattern::Single`]
    /// (class-only); a 2-element array → [`Pattern::Parts`] (class + title, == firmware `WT()`).
    #[serde(rename = "match")]
    pub pattern: Pattern,
    /// The host layer to activate on match — a **raw QMK layer index** (`0..=254`;
    /// `255`/`0xFF` is rejected as the wire "clear" sentinel). `None` (the default
    /// when the key is absent) ⇒ this rule sets no layer. First-match-wins among
    /// layer-setting rules. See spec/HOST_RULES.md §3 C11, §9.
    #[serde(default)]
    pub layer: Option<u8>,
    /// Callback names to enable (run on focus-in). Defaults to empty when absent.
    #[serde(default)]
    pub enable: Vec<String>,
    /// Callback names to disable (force-off override). Defaults to empty when absent.
    /// Order-independent explicit exclusion: any id in ANY matching rule's `disable`
    /// is removed from the union of all `enable`s.
    #[serde(default)]
    pub disable: Vec<String>,
    /// Whether the [`Pattern`] matches case-sensitively. Defaults to `false`.
    #[serde(default)]
    pub case_sensitive: bool,
    /// Per-rule override of [`HostDefaults::disable_firmware_config`]. `None`
    /// (the default when the key is absent) ⇒ inherit the `[host]` global default
    /// (resolved by P3.M1.T1.S2).
    #[serde(default)]
    pub disable_firmware_config: Option<bool>, // None => inherit [host]
}

// ============================================================================
// File-IO + path-resolution + per-rule primitive (P3.M1.T1.S2)
// ============================================================================
// These three functions are the layer between the data model above (S1) and the
// evaluator `evaluate()` (P3.M1.T2.S1): resolve each candidate `rules.toml` path
// (`get_rules_paths`), read + deserialize one into a typed `RuleSet` (`parse_rules`),
// and resolve a single rule's effective `disable_firmware_config` (the primitive).

/// Resolve a single rule's effective `disable_firmware_config`.
///
/// A rule's effective flag is its per-rule override when `Some`, otherwise the
/// `[host]` global default. This is the per-rule input to the stack-vs-replace
/// decision computed by `evaluate()` (P3.M1.T2): the window is "replace" iff
/// EVERY matched rule's effective flag is `true` (HOST_RULES.md §9).
fn effective_disable_firmware_config(rule_override: Option<bool>, host_default: bool) -> bool {
    rule_override.unwrap_or(host_default)
}

/// Read and deserialize a `rules.toml` file into a [`RuleSet`].
///
/// This is the host-side-rules counterpart to [`crate::core::parse_config`]: it reads
/// the file at `path` (via [`fs::read_to_string`]) and deserializes it with
/// [`toml::from_str`]. A missing/unreadable file yields an [`io::Error`](std::io::Error);
/// malformed TOML, or a `[[rule]]` table missing the required `match` key (or
/// setting none of `layer`/`enable`/`disable`), yields a parse error. Both
/// propagate as `Box<dyn Error>` — which is exactly the strict failure
/// `--validate-rules` (P5.M1) reports.
///
/// `path` is a SINGLE candidate (typically the first existing entry of
/// [`get_rules_paths`]); resolving the candidate list is the caller's job, mirroring
/// how `parse_config` is fed by `configured_timing()`'s `.find(|p| p.exists())`.
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::rules::{get_rules_paths, parse_rules};
///
/// if let Some(path) = get_rules_paths().into_iter().find(|p| p.exists()) {
///     let rules = parse_rules(&path)?;   // Err on malformed rules.toml
///     // ... evaluate(rules, window) ...
/// }
/// ```
pub fn parse_rules(path: &Path) -> Result<RuleSet, Box<dyn Error>> {
    let text = fs::read_to_string(path)?;
    let rules: RuleSet = toml::from_str(&text)?;
    validate_rules(&rules)?;
    Ok(rules)
}

/// Validate the `[[rule]]` table: reject the wire "clear" layer sentinel and
/// match-only rules.
///
/// A host layer is a **raw QMK layer index** applied verbatim by the firmware
/// (`layer_on`/`layer_off`, no range check — see spec/HOST_RULES.md §3 C11).
/// The only universally-invalid value is `0xFF` (255): the firmware treats it as
/// `LAYER_UNSET` and silently *clears* the host layer instead of activating one
/// — the exact opposite of the user's intent. Any other byte (`0..=254`) is
/// passed straight through; whether it is a real, addressable layer (within the
/// firmware's `layer_state_t` width and above its board layers) is the user's
/// responsibility — the host cannot know the keymap layout, so it does **not**
/// gate on a fixed floor. (The earlier `[224, 254]` reservation is withdrawn:
/// `layer_state` cannot hold bit 224 even with `LAYER_STATE_32BIT`, and
/// `layer_on(224)` is UB that typically wraps to bit 0.)
///
/// §9 Validity additionally requires that every `[[rule]]` set at least one of
/// `layer`/`enable`/`disable` (in addition to the required `match`): since
/// `layer` is now `Option<u8>` (defaults to `None`), a match-only rule no longer
/// fails deserialization — it must fail HERE instead (same parse boundary that
/// `--validate-rules` and the runtime path rely on).
///
/// Enforced at the parse boundary (the single source of truth) rather than only
/// in `--validate-rules`, so the runtime path also rejects both —
/// `host_context_for_window` then gracefully falls back to string-only mode and
/// `--validate-rules` exits non-zero with this message. Pure (no IO).
fn validate_rules(rules: &RuleSet) -> Result<(), Box<dyn Error>> {
    for rule in &rules.rules {
        // C11: 0xFF is the wire "clear host layer" sentinel — the firmware would
        // silently CLEAR the host layer instead of activating one. Reject it.
        if rule.layer == Some(0xFF) {
            return Err(
                "invalid [[rule]] layer 255: 0xFF is the wire \"clear host layer\" sentinel — the \
                 firmware would silently clear the host layer instead of activating one. \
                 Use a real QMK layer index (0..=254). See spec/HOST_RULES.md §3 C11"
                    .into(),
            );
        }
        // §9 Validity: a rule must set at least one of layer/enable/disable in
        // addition to the required match. (Since `layer` is now Option, a
        // match-only rule no longer fails deserialization — it must fail HERE.)
        if rule.layer.is_none() && rule.enable.is_empty() && rule.disable.is_empty() {
            return Err(
                "invalid rule: must set at least one of layer/enable/disable (in \
                 addition to match). See spec/HOST_RULES.md §9 Validity"
                    .into(),
            );
        }
    }
    Ok(())
}

/// Return the candidate `rules.toml` paths, in platform preference order.
///
/// `rules.toml` lives **alongside `config.toml`** (HOST_RULES.md §8): same
/// directory, swapped filename. This function derives the list by delegating to
/// [`crate::platforms::get_config_paths`] and swapping each entry's final
/// filename component to `rules.toml` (via [`std::path::Path::with_file_name`]).
///
/// On Linux this is `$XDG_CONFIG_HOME/qmkonnect/rules.toml`,
/// `~/.config/qmkonnect/rules.toml`, `/etc/qmkonnect/rules.toml`; on
/// macOS `~/Library/Application Support/QMKonnect/rules.toml` (+ fallbacks); on
/// Windows `%APPDATA%\QMKonnect\rules.toml` (+ fallbacks). An absent file at
/// every candidate ⇒ the caller disables host rules (string-only, legacy path).
pub fn get_rules_paths() -> Vec<PathBuf> {
    crate::platforms::get_config_paths()
        .into_iter()
        .map(|p| p.with_file_name("rules.toml"))
        .collect()
}

// ============================================================================
// Validation helpers surfaced by `--validate-rules` (P5.M1)
// ============================================================================
// Pure functions over a parsed `RuleSet` that flag *configurable* mistakes the
// strict parse cannot catch (a well-formed but semantically surprising rule).
// `--validate-rules` turns these into warnings; they never fail the parse (the
// behaviour they describe is intentional and spec-compliant) — they only help
// the user spot footguns.

/// Callback names that a SINGLE rule both `enable`s and `disable`s.
///
/// Such a name is a contradictory no-op: the one-pass evaluator resolves it to
/// DISABLED (the explicit-exclusion override wins), so the `enable` entry is
/// dead. Surfaced as a warning by `--validate-rules` (#8). Pure + deterministic
/// (deduped + sorted via `BTreeSet`).
///
/// # Example
///
/// ```rust,ignore
/// use qmkonnect::core::rules::contradictory_callback_names;
/// let rules: RuleSet = toml::from_str(r#"
/// [[rule]]
/// match = "a"
/// enable = ["foo"]
/// disable = ["foo", "bar"]
/// "#).unwrap();
/// assert_eq!(contradictory_callback_names(&rules), vec!["foo".to_string()]);
/// ```
pub fn contradictory_callback_names(rules: &RuleSet) -> Vec<String> {
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for rule in &rules.rules {
        let dis: BTreeSet<&str> = rule.disable.iter().map(|s| s.as_str()).collect();
        for name in &rule.enable {
            if dis.contains(name.as_str()) {
                seen.insert(name.clone());
            }
        }
    }
    seen.into_iter().collect()
}

/// Does this pattern's match core reduce to empty?
///
/// `match = ""` (a `Single("")`) — or a `Parts` with an empty class/title half
/// — hits the firmware-parity empty-core short-circuit, which matches ONLY
/// windows whose class/title string is empty, NOT "all windows" (verified
/// firmware parity, `pattern.rs`). Users wanting a catch-all must write
/// `match = "*"`. Surfaced as a warning by `--validate-rules` (#9). Pure.
pub fn pattern_is_empty_core(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Single(s) => s.is_empty(),
        Pattern::Parts(c, t) => c.is_empty() || t.is_empty(),
    }
}

// ============================================================================
// The pure per-window evaluator: given a parsed `RuleSet` + the window
// (app_class, title) + the handshake name→id map + whether the board has its
// own rules, decide the host layer, the desired enabled callback id set, the
// `clear_board` (stack-vs-replace) flag, and whether any rule matched.
// Consumes `RuleSet`/`HostDefaults`/`LayerRule`/`CallbackRule` (S1), the private
// `effective_disable_firmware_config` primitive (S2), and `match_pattern` (P2).

/// The result of evaluating host `rules.toml` against one window — the single
/// packet the `notify_qmk` send logic (P4.M3.T1.S1) consumes.
///
/// Fields (HOST_RULES.md §8(3) / §4):
/// - `layer`: the first matching rule's `layer` (`L_h`, a raw QMK layer index),
///   or `None` when no layer-setting rule matched (firmware maps `None` to `0xFF`).
/// - `callback_ids`: the **desired enabled** callback id set — the union of every
///   matching rule's `enable` names (resolved through the handshake
///   `name_to_id` map) MINUS each rule's `disable` names (explicit exclusion).
///   Sorted (built from a `BTreeSet`); empty when no callback matched.
/// - `clear_board`: the stack-vs-replace bit. `true` (replace) iff every matched
///   rule's effective `disable_firmware_config` is `true` **or** the board has no
///   rules of its own; `false` (stack) otherwise. Always `false` on no-match.
/// - `any_match`: `true` iff at least one rule matched.
///
/// Downstream: `send_string = board_has_rules && any_match && !clear_board`;
/// the wire payload is `ApplyHostContext { layer, callbacks: callback_ids, clear_board }`.
#[derive(Debug, Clone, PartialEq)]
pub struct HostContext {
    pub layer: Option<u8>,
    pub callback_ids: Vec<u8>,
    pub clear_board: bool,
    pub any_match: bool,
}

/// Evaluate host `rules.toml` against one window and produce a [`HostContext`].
///
/// One pass over `[[rule]]` in file order (HOST_RULES.md §8(3)):
///
/// For each matching rule (there is no `break` — all matches must accumulate):
/// - if it sets `layer` and none is chosen yet, that layer wins
///   (first-match-wins, **exclusive** — one host layer). Callback-only rules
///   (`layer == None`) are skipped for the layer decision but still contribute
///   their `enable`/`disable` names.
/// - `enable` names (resolved via `name_to_id`) accumulate into an enable set and
///   `disable` names into a disable set. After the loop they are differenced
///   ONCE so `disable` is an **order-independent explicit-exclusion override**
///   (HOST_RULES.md §4/§9, §13 Q2): it always wins, regardless of rule order.
///   Unknown names are skipped silently — validation/warning is the handshake's
///   job (P4.M2).
/// - one effective flag is pushed per matched RULE (so a single `[[rule]]` setting
///   both `layer` and `enable` pushes one flag, exactly as its old-schema
///   equivalent — a layer rule + a callback rule with the same effective flag —
///   pushed two identical flags; `all()` yields the same result either way).
///
/// Stack-vs-replace: `clear_board = true` iff every matched rule's effective
/// `disable_firmware_config` is `true` **or** `board_has_rules` is `false`
/// (HOST_RULES.md §4: "replace = all-disabling OR board-has-no-rules").
///
/// **No match** (no rule matched) short-circuits to
/// `{ layer: None, callback_ids: vec![], clear_board: false, any_match: false }`
/// — `clear_board` is literally `false` (C13: a host no-match NEVER suppresses
/// the board; the host clears only its own layer/callbacks). The global
/// `[host].disable_firmware_config` default affects matched windows only.
///
/// This function is **pure** — no IO, no logging, no global state.
///
/// # Example
///
/// ```rust,ignore
/// use std::collections::HashMap;
/// use qmkonnect::core::rules::{evaluate, parse_rules, get_rules_paths};
///
/// let rules = parse_rules(&get_rules_paths().into_iter().find(|p| p.exists()).unwrap()).unwrap();
/// let mut name_to_id = HashMap::new();
/// name_to_id.insert("vim_lazy".to_string(), 0u8);
/// let ctx = evaluate(&rules, "Alacritty", "vim", &name_to_id, /* board_has_rules */ true);
/// // ctx.clear_board => send only ApplyHostContext; !ctx.clear_board => send string first.
/// ```
pub fn evaluate(
    rules: &RuleSet,
    app_class: &str,
    title: &str,
    name_to_id: &HashMap<String, u8>,
    board_has_rules: bool,
) -> HostContext {
    let host_default = rules.host.disable_firmware_config;

    let mut layer: Option<u8> = None; // first layer-setting match wins (exclusive)
    let mut matched_effective: Vec<bool> = Vec::new(); // one flag PER MATCHED RULE
    let mut enabled: BTreeSet<u8> = BTreeSet::new();
    let mut disabled: BTreeSet<u8> = BTreeSet::new();

    // ONE pass over [[rule]] (file order). For each matching rule: push its
    // effective flag (once); set layer if this rule sets one and none chosen
    // yet (first-match-wins, exclusive — one host layer); accumulate enable
    // names → enabled set, disable names → disabled set (all-match). A rule may
    // set layer only, callbacks only, or both (spec §8(3)).
    for rule in &rules.rules {
        if match_pattern(&rule.pattern, app_class, title, rule.case_sensitive) {
            matched_effective.push(effective_disable_firmware_config(
                rule.disable_firmware_config,
                host_default,
            ));
            if rule.layer.is_some() && layer.is_none() {
                layer = rule.layer; // first-match-wins, exclusive
            }
            for name in &rule.enable {
                if let Some(&id) = name_to_id.get(name) {
                    enabled.insert(id);
                } // else: unknown name -> skip silently (handshake warns, P4.M2)
            }
            for name in &rule.disable {
                if let Some(&id) = name_to_id.get(name) {
                    disabled.insert(id);
                }
            }
        }
    }

    // Disable wins regardless of rule order: difference removes any id present
    // in ANY matching rule's `disable` from the union of all `enable`s (two-set
    // difference = order-independent explicit-exclusion override, §4/§9).
    let desired: BTreeSet<u8> = enabled.difference(&disabled).copied().collect();

    // No match -> short-circuit BEFORE the formula (all() is vacuously true on
    // an empty Vec, which would wrongly yield clear_board=true). C13: a host
    // no-match NEVER suppresses the board — the host clears only its own
    // layer/callbacks; the board silo still runs.
    if matched_effective.is_empty() {
        return HostContext {
            layer: None,
            callback_ids: vec![],
            clear_board: false,
            any_match: false,
        };
    }

    // Stack-vs-replace: replace iff every matched rule is disabling OR no board rules.
    let all_disabling = matched_effective.iter().all(|&f| f);
    let clear_board = all_disabling || !board_has_rules;

    HostContext {
        layer,
        callback_ids: desired.into_iter().collect(), // sorted (BTreeSet)
        clear_board,
        any_match: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The verbatim `spec/HOST_RULES.md` §9 example (see research/notes.md §2).
    const SECTION_9_TOML: &str = r#"[host]
disable_firmware_config = false

[[layer_rules]]
match = "alacritty"
layer = 224
disable_firmware_config = true

[[layer_rules]]
match = ["*chrome*", "*youtube*"]
layer = 225
case_sensitive = false

[[callback_rules]]
match = "neovide"
enable = ["vim_lazy", "disable_vim"]
disable = ["vim_lazy"]

[[callback_rules]]
match = ["*chrome*", "*claude*"]
enable = ["vim_lazy", "disable_vim"]
disable_firmware_config = true
"#;

    #[test]
    fn test_rules_full_section9_example_parses() {
        let rs: RuleSet = toml::from_str(SECTION_9_TOML).unwrap();

        // [host] defaults
        assert!(!(rs.host.disable_firmware_config));

        // layer_rules: first-match-wins ordering preserved.
        assert_eq!(rs.layer_rules.len(), 2);

        // layer_rules[0]: class-only Single + explicit override.
        assert_eq!(
            rs.layer_rules[0].pattern,
            Pattern::Single("alacritty".into())
        );
        assert_eq!(rs.layer_rules[0].layer, 224);
        assert!(!(rs.layer_rules[0].case_sensitive));
        assert_eq!(rs.layer_rules[0].disable_firmware_config, Some(true));

        // layer_rules[1]: class+title Parts + no override (inherits [host]).
        assert_eq!(
            rs.layer_rules[1].pattern,
            Pattern::Parts("*chrome*".into(), "*youtube*".into())
        );
        assert_eq!(rs.layer_rules[1].layer, 225);
        assert!(!(rs.layer_rules[1].case_sensitive));
        assert_eq!(rs.layer_rules[1].disable_firmware_config, None);

        // callback_rules: all-match ordering preserved.
        assert_eq!(rs.callback_rules.len(), 2);

        // callback_rules[0]: enable + disable lists, no override.
        assert_eq!(
            rs.callback_rules[0].pattern,
            Pattern::Single("neovide".into())
        );
        assert_eq!(
            rs.callback_rules[0].enable,
            vec!["vim_lazy".to_string(), "disable_vim".to_string()]
        );
        assert_eq!(rs.callback_rules[0].disable, vec!["vim_lazy".to_string()]);
        assert!(!(rs.callback_rules[0].case_sensitive));
        assert_eq!(rs.callback_rules[0].disable_firmware_config, None);

        // callback_rules[1]: Parts + override.
        assert_eq!(
            rs.callback_rules[1].pattern,
            Pattern::Parts("*chrome*".into(), "*claude*".into())
        );
        assert_eq!(
            rs.callback_rules[1].enable,
            vec!["vim_lazy".to_string(), "disable_vim".to_string()]
        );
        assert_eq!(rs.callback_rules[1].disable, Vec::<String>::new());
        assert_eq!(rs.callback_rules[1].disable_firmware_config, Some(true));
    }

    #[test]
    fn test_rules_missing_host_table_defaults_false() {
        // A rules.toml with rules but no [host] table: #[serde(default)] +
        // manual HostDefaults::default() => disable_firmware_config == false.
        let toml = r#"
[[layer_rules]]
match = "firefox"
layer = 224
"#;
        let rs: RuleSet = toml::from_str(toml).unwrap();
        assert!(!(rs.host.disable_firmware_config));
        assert_eq!(rs.layer_rules.len(), 1);
    }

    #[test]
    fn test_rules_empty_toml_is_all_default() {
        let rs: RuleSet = toml::from_str("").unwrap();
        assert!(!(rs.host.disable_firmware_config));
        assert!(rs.layer_rules.is_empty());
        assert!(rs.callback_rules.is_empty());
        // Equivalent to RuleSet::default().
        assert_eq!(
            rs.host.disable_firmware_config,
            RuleSet::default().host.disable_firmware_config
        );
    }

    #[test]
    fn test_rules_layer_override_absent_is_none() {
        // A [[layer_rules]] without disable_firmware_config => None (inherit host, G5).
        let toml = r#"
[[layer_rules]]
match = "kitty"
layer = 230
"#;
        let rs: RuleSet = toml::from_str(toml).unwrap();
        assert_eq!(rs.layer_rules[0].disable_firmware_config, None);
    }

    #[test]
    fn test_rules_callback_enable_disable_default_empty() {
        // A [[callback_rules]] with only match + enable => disable == vec![].
        let toml = r#"
[[callback_rules]]
match = "wezterm"
enable = ["vim_lazy"]
"#;
        let rs: RuleSet = toml::from_str(toml).unwrap();
        assert_eq!(rs.callback_rules.len(), 1);
        assert_eq!(rs.callback_rules[0].enable, vec!["vim_lazy".to_string()]);
        assert!(rs.callback_rules[0].disable.is_empty());

        // And vice versa: only disable => enable == vec![].
        let toml2 = r#"
[[callback_rules]]
match = "wezterm"
disable = ["vim_lazy"]
"#;
        let rs2: RuleSet = toml::from_str(toml2).unwrap();
        assert!(rs2.callback_rules[0].enable.is_empty());
        assert_eq!(rs2.callback_rules[0].disable, vec!["vim_lazy".to_string()]);
    }

    #[test]
    fn test_rules_match_string_to_single_and_array_to_parts() {
        // match = "x" => Pattern::Single (delegates to Pattern's untagged serde).
        let single = r#"
[[layer_rules]]
match = "x"
layer = 224
"#;
        let rs: RuleSet = toml::from_str(single).unwrap();
        assert_eq!(rs.layer_rules[0].pattern, Pattern::Single("x".into()));

        // match = ["a","b"] => Pattern::Parts (delegates to Pattern's untagged serde).
        let parts = r#"
[[layer_rules]]
match = ["a", "b"]
layer = 224
"#;
        let rs: RuleSet = toml::from_str(parts).unwrap();
        assert_eq!(
            rs.layer_rules[0].pattern,
            Pattern::Parts("a".into(), "b".into())
        );
    }

    #[test]
    fn test_rules_missing_layer_errors() {
        // A [[layer_rules]] with match but no layer is a deserialization error (G6).
        let toml = r#"
[[layer_rules]]
match = "x"
"#;
        let res = toml::from_str::<RuleSet>(toml);
        assert!(res.is_err(), "expected error when `layer` is missing");
    }

    #[test]
    fn test_rules_missing_match_errors() {
        // A [[layer_rules]] with layer but no match is a deserialization error (G6).
        let toml = r#"
[[layer_rules]]
layer = 224
"#;
        let res = toml::from_str::<RuleSet>(toml);
        assert!(res.is_err(), "expected error when `match` is missing");
    }

    #[test]
    fn test_rules_default_propagates() {
        // RuleSet::default() proves Default propagation: manual HostDefaults::default()
        // (=> false) + empty Vecs (G4).
        let rs = RuleSet::default();
        assert!(!(rs.host.disable_firmware_config));
        assert!(rs.layer_rules.is_empty());
        assert!(rs.callback_rules.is_empty());
    }

    // ========================================================================
    // P3.M1.T1.S2 — effective_disable_firmware_config + parse_rules + get_rules_paths
    // ========================================================================

    // ---- effective_disable_firmware_config: the 4-row truth table (G5) ----

    #[test]
    fn test_rules_effective_some_true_wins() {
        // Some(true) overrides host_default=false.
        assert!(effective_disable_firmware_config(Some(true), false));
    }

    #[test]
    fn test_rules_effective_some_false_wins() {
        // Some(false) overrides host_default=true.
        assert!(!effective_disable_firmware_config(Some(false), true));
    }

    #[test]
    fn test_rules_effective_none_inherits_false() {
        // None inherits host_default=false.
        assert!(!effective_disable_firmware_config(None, false));
    }

    #[test]
    fn test_rules_effective_none_inherits_true() {
        // None inherits host_default=true.
        assert!(effective_disable_firmware_config(None, true));
    }

    // ---- parse_rules: end-to-end file IO (tempfile, G9 single-threaded) ----

    #[test]
    fn test_rules_parse_valid_section9() {
        // The verbatim HOST_RULES.md §9 example round-trips through a real file:
        // fs::read_to_string -> toml::from_str -> RuleSet (G3 single path, G4 err type).
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, SECTION_9_TOML).unwrap();

        let rs = parse_rules(&path).unwrap();

        // [host] default
        assert!(!(rs.host.disable_firmware_config));

        // layer_rules[0]: class-only Single + explicit override.
        assert_eq!(rs.layer_rules.len(), 2);
        assert_eq!(rs.layer_rules[0].layer, 224);
        assert_eq!(rs.layer_rules[0].disable_firmware_config, Some(true));

        // callback_rules: all-match ordering preserved.
        assert_eq!(rs.callback_rules.len(), 2);
    }

    #[test]
    fn test_rules_parse_missing_file_errors() {
        // A nonexistent path => fs::read_to_string io::Error propagates as Err.
        let p = Path::new("/nonexistent/qmk-rules-xyz-9f8e7.toml");
        assert!(parse_rules(p).is_err());
    }

    #[test]
    fn test_rules_parse_malformed_toml_errors() {
        // Genuinely malformed TOML => toml::de::Error propagates as Err.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, "not = valid = toml = =").unwrap();

        assert!(parse_rules(&path).is_err());
    }

    #[test]
    fn test_rules_parse_missing_required_field_errors() {
        // A [[layer_rules]] with `match` but no required `layer` surfaces S1's
        // required-field strictness through the file path — the contract that
        // `--validate-rules` (P5.M1) will report.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(
            &path,
            r#"
[[layer_rules]]
match = "x"
"#,
        )
        .unwrap();

        assert!(parse_rules(&path).is_err());
    }

    // ---- get_rules_paths: transformation invariant (G10 env-independent) ----

    #[test]
    fn test_rules_paths_swap_filename() {
        // The core contract: every rules path is the config path with the
        // filename swapped to `rules.toml` in the SAME directory. Asserted
        // against the real platform resolver output — no env mutation (G10).
        // Robust on every platform, including the empty-Vec unknown-platform
        // case (zip of two empty iterators runs zero iterations, len==len==0).
        let cfg = crate::platforms::get_config_paths();
        let rul = get_rules_paths();

        assert_eq!(cfg.len(), rul.len(), "delegate must preserve path count");
        for (c, r) in cfg.iter().zip(rul.iter()) {
            assert_eq!(
                c.parent(),
                r.parent(),
                "rules.toml must be in the SAME dir as config.toml"
            );
            assert_eq!(r.file_name(), Some(std::ffi::OsStr::new("rules.toml")));
            assert_eq!(c.file_name(), Some(std::ffi::OsStr::new("config.toml")));
        }
    }

    #[test]
    #[cfg(any(target_os = "linux", target_os = "macos", target_os = "windows"))]
    fn test_rules_paths_delegate_count() {
        // Sanity that delegation returned real paths on supported platforms.
        // (test_rules_paths_swap_filename's len-equality already implies this;
        // this is an explicit positive assertion behind a cfg guard so it never
        // falsely fails on a future non-Linux/macOS/Windows CI target.)
        assert!(
            !get_rules_paths().is_empty(),
            "supported platform should return at least one rules.toml candidate"
        );
    }

    // ========================================================================
    // P3.M1.T2.S1 — evaluate() + HostContext
    // ========================================================================
    // The three-stage per-window evaluator: layer first-match → L_h; callbacks
    // all-match → desired id set (enable-union / disable-exclusion); stack-vs-
    // replace → clear_board. ~14 tests below cover each stage + the truth table.

    /// Helper: build a name→id map from (&str, u8) pairs.
    fn name_map(pairs: &[(&str, u8)]) -> HashMap<String, u8> {
        pairs.iter().map(|(n, id)| (n.to_string(), *id)).collect()
    }

    // ---- A. Basic / no-match (G2 no-match early-return) ----

    #[test]
    fn test_evaluate_empty_ruleset_no_match() {
        // RuleSet::default() against any window -> { None, vec![], false, false }.
        let rules = RuleSet::default();
        let n2i = name_map(&[("vim_lazy", 0)]);
        let ctx = evaluate(&rules, "Alacritty", "vim", &n2i, true);
        assert_eq!(
            ctx,
            HostContext {
                layer: None,
                callback_ids: vec![],
                clear_board: false,
                any_match: false,
            }
        );
    }

    #[test]
    fn test_evaluate_no_layer_no_callback_match() {
        // Rules present but no pattern matches the window -> no-match early-return.
        let toml = r#"
[[layer_rules]]
match = "firefox"
layer = 224

[[callback_rules]]
match = "neovide"
enable = ["vim_lazy"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("vim_lazy", 0)]);
        let ctx = evaluate(&rules, "Alacritty", "vim", &n2i, true);
        assert_eq!(
            ctx,
            HostContext {
                layer: None,
                callback_ids: vec![],
                clear_board: false,
                any_match: false,
            }
        );
    }

    /// C13 regression: a host no-match NEVER suppresses the board. Even with the
    /// global `[host].disable_firmware_config = true` default, a no-match window
    /// returns `clear_board: false` (the host clears only its own
    /// layer/callbacks; the board silo still runs). The global default now
    /// affects matched windows only.
    #[test]
    fn test_evaluate_no_match_clear_board_always_false() {
        let toml = r#"
[host]
disable_firmware_config = true

[[layer_rules]]
match = "firefox"
layer = 224
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[]);
        let ctx = evaluate(&rules, "Alacritty", "vim", &n2i, true);
        assert_eq!(
            ctx,
            HostContext {
                layer: None,
                callback_ids: vec![],
                clear_board: false, // C13: host no-match never clears the board
                any_match: false,
            }
        );
    }

    #[test]
    fn test_evaluate_layer_first_match_wins() {
        // Two layer rules both match (Single "a"); the first's layer wins and
        // the second is never consulted. Give them DIFFERENT layers to prove it.
        let toml = r#"
[[layer_rules]]
match = "a"
layer = 224

[[layer_rules]]
match = "a"
layer = 225
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "anything", &n2i, true);
        assert_eq!(ctx.layer, Some(224));
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_layer_second_when_first_misses() {
        // First pattern misses ("zzz"), second ("a") matches -> second.layer wins.
        let toml = r#"
[[layer_rules]]
match = "zzz"
layer = 224

[[layer_rules]]
match = "a"
layer = 230
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "anything", &n2i, true);
        assert_eq!(ctx.layer, Some(230));
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_layer_parts_requires_both_halves() {
        // Pattern::Parts(["a","b"]) with app_class "a" but title "x" -> the
        // title half fails, so NO match (layer stays None, any_match false).
        let toml = r#"
[[layer_rules]]
match = ["a", "b"]
layer = 224
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "x", &n2i, true);
        assert_eq!(ctx.layer, None);
        assert!(!ctx.any_match);
    }

    // ---- C. Callbacks (all-match + enable/disable) ----

    #[test]
    fn test_evaluate_callback_all_matches_union() {
        // Two callback rules both match, each enabling a disjoint name -> the
        // desired set is the UNION of both.
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["x"]

[[callback_rules]]
match = "a"
enable = ["y"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1), ("y", 2)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![1, 2]);
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_callback_disable_is_exclusion() {
        // Rule A enables "x", rule B (also matches) disables "x" -> x is absent
        // from callback_ids (explicit-exclusion override).
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["x"]

[[callback_rules]]
match = "a"
disable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![] as Vec<u8>);
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_unknown_name_skipped() {
        // A rule enables a name NOT in name_to_id ("ghost") alongside a known
        // one ("x"). No panic; "ghost" contributes nothing, "x" still resolves.
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["x", "ghost"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![1]);
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_callback_ids_sorted() {
        // Insert ids {3,1,2} across rules in a deliberately non-sorted order ->
        // callback_ids == vec![1,2,3] (BTreeSet determinism, G3).
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["c"]

[[callback_rules]]
match = "a"
enable = ["a"]

[[callback_rules]]
match = "a"
enable = ["b"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("a", 1), ("b", 2), ("c", 3)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![1, 2, 3]);
    }

    // ---- D. clear_board truth table (G1 formula + G2 no-match guard) ----

    #[test]
    fn test_evaluate_clear_board_all_disabling() {
        // Sole matched rule effective=true (override Some(true)) -> clear_board=true
        // even with board_has_rules=true (replace: all-disabling).
        let toml = r#"
[[layer_rules]]
match = "a"
layer = 224
disable_firmware_config = true
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.layer, Some(224));
        assert!(ctx.clear_board); // all-disabling
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_clear_board_one_nondisabling_is_false() {
        // One matched rule effective=false (override Some(false), host default
        // true) -> clear_board=false (stack), board_has_rules=true.
        let toml = r#"
[host]
disable_firmware_config = true

[[layer_rules]]
match = "a"
layer = 224
disable_firmware_config = false
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert!(!ctx.clear_board); // NOT all-disabling -> stack
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_clear_board_no_board_rules() {
        // board_has_rules=false -> clear_board=true even if the matched rule is
        // non-disabling (effective=false): replace because nothing to stack onto.
        let toml = r#"
[[layer_rules]]
match = "a"
layer = 224
disable_firmware_config = false
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = HashMap::new();
        let ctx = evaluate(&rules, "a", "t", &n2i, /* board_has_rules */ false);
        assert!(ctx.clear_board); // !board_has_rules -> replace
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_effective_inherits_host_default() {
        // rule.disable_firmware_config=None -> effective = host_default.
        //  (a) host=false -> effective false -> clear_board=false (stack).
        //  (b) host=true  -> effective true  -> clear_board=true  (replace).
        // (RuleSet isn't Clone, so parse two fresh copies with the [host] bit flipped.)
        let toml_a = r#"
[host]
disable_firmware_config = false

[[layer_rules]]
match = "a"
layer = 224
"#;
        let toml_b = r#"
[host]
disable_firmware_config = true

[[layer_rules]]
match = "a"
layer = 224
"#;
        let n2i = HashMap::new();

        // (a) host=false
        let rules_a: RuleSet = toml::from_str(toml_a).unwrap();
        let ctx_a = evaluate(&rules_a, "a", "t", &n2i, true);
        assert!(!ctx_a.clear_board);

        // (b) host=true
        let rules_b: RuleSet = toml::from_str(toml_b).unwrap();
        let ctx_b = evaluate(&rules_b, "a", "t", &n2i, true);
        assert!(ctx_b.clear_board);
    }

    // ---- E. Cross-stage ----

    #[test]
    fn test_evaluate_layer_match_callback_miss() {
        // Layer matches, no callback matches -> layer set, callback_ids empty,
        // any_match=true (and, board_has_rules=true + non-disabling default,
        // clear_board=false).
        let toml = r#"
[[layer_rules]]
match = "a"
layer = 224

[[callback_rules]]
match = "zzz"
enable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.layer, Some(224));
        assert_eq!(ctx.callback_ids, vec![] as Vec<u8>);
        assert!(ctx.any_match);
        assert!(!ctx.clear_board); // default host=false, rule None -> effective false
    }

    #[test]
    fn test_evaluate_callback_match_layer_miss() {
        // Mirror of the above: callback matches, layer misses -> layer None,
        // callback_ids populated, any_match=true.
        let toml = r#"
[[layer_rules]]
match = "zzz"
layer = 224

[[callback_rules]]
match = "a"
enable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.layer, None);
        assert_eq!(ctx.callback_ids, vec![1]);
        assert!(ctx.any_match);
    }

    // ========================================================================
    // disable order-independence (#1): disable is an explicit-exclusion
    // override that wins regardless of whether its rule precedes or follows a
    // matching enable. The earlier single-pass insert-then-remove made the
    // exclusion last-writer-wins.
    // ========================================================================

    #[test]
    fn test_evaluate_disable_after_enable_excludes() {
        // The order the spec example implies: enable first, disable second.
        // x is absent (explicit-exclusion override). [baseline — already worked]
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["x"]

[[callback_rules]]
match = "a"
disable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![] as Vec<u8>);
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_disable_before_enable_still_excludes() {
        // #1 regression: disable listed BEFORE a later matching enable. The
        // previous implementation re-enabled x (last-writer-wins); the two-pass
        // difference makes disable win, so x stays excluded.
        let toml = r#"
[[callback_rules]]
match = "a"
disable = ["x"]

[[callback_rules]]
match = "a"
enable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(
            ctx.callback_ids, vec![] as Vec<u8>,
            "disable must win regardless of rule order (order-independent exclusion)"
        );
        assert!(ctx.any_match);
    }

    #[test]
    fn test_evaluate_disable_excludes_only_named_others_survive() {
        // A global "disable x" guard does not suppress unrelated enables: y/z
        // survive while only x is excluded, no matter where the guard sits.
        let toml = r#"
[[callback_rules]]
match = "a"
disable = ["x"]

[[callback_rules]]
match = "a"
enable = ["x", "y", "z"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        let n2i = name_map(&[("x", 1), ("y", 2), ("z", 3)]);
        let ctx = evaluate(&rules, "a", "t", &n2i, true);
        assert_eq!(ctx.callback_ids, vec![2, 3]);
    }

    // ========================================================================
    // Layer validation: only the 0xFF "clear" sentinel is rejected. Any other
    // byte (0..=254) is a valid raw QMK layer index — there is no fixed floor
    // (the old [224, 254] reservation is withdrawn; see spec/HOST_RULES.md C11).
    // ========================================================================

    #[test]
    fn test_parse_rules_rejects_layer_255_clear_sentinel() {
        // 255 maps to wire 0xFF (LAYER_UNSET/clear) — reject so the user is
        // told instead of having the host layer silently cleared.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(&path, "[[layer_rules]]\nmatch = \"a\"\nlayer = 255\n").unwrap();
        let res = parse_rules(&path);
        assert!(res.is_err());
        let msg = res.unwrap_err().to_string();
        assert!(msg.contains("255"), "error must name the bad layer: {msg}");
        assert!(
            msg.contains("clear"),
            "error must explain 255 is the clear sentinel: {msg}"
        );
    }

    #[test]
    fn test_parse_rules_accepts_low_layer_indices() {
        // There is no fixed floor: 0, a real board-style index (28), 100, the
        // former floor (224), and the former ceiling (254) are all valid raw
        // QMK layer indices.
        for &layer in &[0u8, 28, 100, 224, 254] {
            let dir = tempfile::TempDir::new().unwrap();
            let path = dir.path().join("rules.toml");
            std::fs::write(&path, format!("[[layer_rules]]\nmatch = \"a\"\nlayer = {layer}\n")).unwrap();
            let res = parse_rules(&path);
            assert!(res.is_ok(), "layer {layer} should be valid");
        }
    }

    #[test]
    fn test_parse_rules_reports_first_bad_layer() {
        // The only bad value is 255. A valid low layer followed by 255 must
        // report the 255 (the first — and only — bad rule), not a silent accept.
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("rules.toml");
        std::fs::write(
            &path,
            "[[layer_rules]]\nmatch = \"a\"\nlayer = 5\n\n[[layer_rules]]\nmatch = \"b\"\nlayer = 255\n",
        )
        .unwrap();
        let res = parse_rules(&path);
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("255"));
    }

    // ========================================================================
    // --validate-rules warning helpers (#8 contradictory, #9 empty pattern)
    // ========================================================================

    #[test]
    fn test_contradictory_callback_names_flags_same_rule_overlap() {
        // A rule that both enables and disables "foo" -> "foo" is flagged (#8).
        // The disable-list-only "bar" is NOT contradictory (no matching enable).
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["foo", "bar"]
disable = ["foo"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        assert_eq!(contradictory_callback_names(&rules), vec!["foo".to_string()]);
    }

    #[test]
    fn test_contradictory_callback_names_cross_rule_is_not_contradictory() {
        // enable in rule A + disable in rule B (different rules) is NOT a
        // contradiction — it is the legitimate explicit-exclusion override.
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["x"]

[[callback_rules]]
match = "a"
disable = ["x"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        assert!(contradictory_callback_names(&rules).is_empty());
    }

    #[test]
    fn test_contradictory_callback_names_deduped_sorted() {
        // The same name contradicted in two rules is reported once; output is
        // sorted (BTreeSet).
        let toml = r#"
[[callback_rules]]
match = "a"
enable = ["z", "m"]
disable = ["z", "m"]
"#;
        let rules: RuleSet = toml::from_str(toml).unwrap();
        assert_eq!(
            contradictory_callback_names(&rules),
            vec!["m".to_string(), "z".to_string()]
        );
    }

    #[test]
    fn test_pattern_is_empty_core_single() {
        // #9: match = "" -> empty core -> matches only empty-class windows.
        assert!(pattern_is_empty_core(&Pattern::Single("".into())));
        assert!(!pattern_is_empty_core(&Pattern::Single("*".into())));
        assert!(!pattern_is_empty_core(&Pattern::Single("alacritty".into())));
    }

    #[test]
    fn test_pattern_is_empty_core_parts() {
        // Either empty half of a Parts is the same footgun (that half matches
        // only the empty string).
        assert!(pattern_is_empty_core(&Pattern::Parts("".into(), "*".into())));
        assert!(pattern_is_empty_core(&Pattern::Parts("*".into(), "".into())));
        assert!(!pattern_is_empty_core(&Pattern::Parts("*".into(), "*".into())));
    }
}
