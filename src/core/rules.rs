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
//! [`spec/HOST_RULES.md`] §9. [`Pattern`] is imported (not redefined) from
//! [`crate::core::pattern`] (P2.M1.T3.S2).

// These structs are the data model for the host-rules system. They are exercised
// by the deserialization tests below and will be consumed cross-module by
// P3.M1.T1.S2 (`parse_rules` + effective resolution) and P3.M1.T2.S1
// (`evaluate`). Until those land, non-test builds flag them as unused —
// allow that here rather than at each call site (same idiom as pattern.rs:15).
#![allow(dead_code)]

use crate::core::pattern::Pattern; // P2.M1.T3.S2 — Single/Parts, #[serde(untagged)]
use serde::Deserialize;

/// The top-level `rules.toml` model — a `[host]` defaults table plus two
/// table-arrays (`[[layer_rules]]` and `[[callback_rules]]`).
///
/// Every field is `#[serde(default)]`, so a partial/empty `rules.toml` parses to
/// an all-default ruleset (host stack default, empty rule vectors). See
/// `spec/HOST_RULES.md` §9.
///
/// ```toml
/// [host]
/// disable_firmware_config = false   # global default: false = stack (board runs), true = replace
/// # On no match the host layer is always cleared and all host callbacks disabled.
///
/// # Layer rules: FIRST match wins. One host layer active at a time (>= 224).
/// [[layer_rules]]
/// match = "alacritty"                       # class-only pattern
/// layer = 224
/// disable_firmware_config = true           # optional override (default inherits [host])
///
/// [[layer_rules]]
/// match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT())
/// layer = 225
/// case_sensitive = false                    # optional, default false
///
/// # Callback rules: ALL matches fire. Names come from the keyboard's registry
/// # (run `qmkonnect --list-callbacks` to see them).
/// [[callback_rules]]
/// match = "neovide"
/// enable = ["vim_lazy", "disable_vim"]      # run on focus-in
/// disable = ["vim_lazy"]                    # optional: force-off override
///
/// [[callback_rules]]
/// match = ["*chrome*", "*claude*"]
/// enable = ["vim_lazy", "disable_vim"]
/// disable_firmware_config = true           # for this window, skip the string -> board can't match
/// ```
#[derive(Debug, Deserialize, Default)]
pub struct RuleSet {
    /// Global host defaults applied to every rule that does not override them.
    #[serde(default)]
    pub host: HostDefaults,
    /// Ordered layer rules. Evaluation is **first-match-wins**
    /// (P3.M1.T2.S1); one host layer is active at a time and must be `>= 224`.
    #[serde(default, rename = "layer_rules")]
    pub layer_rules: Vec<LayerRule>,
    /// Ordered callback rules. Evaluation fires **all matches**
    /// (P3.M1.T2.S1); each may `enable`/`disable` callbacks by registry name.
    #[serde(default, rename = "callback_rules")]
    pub callback_rules: Vec<CallbackRule>,
}

/// The `[host]` table — global defaults applied to every rule that does not
/// explicitly override them.
///
/// `disable_firmware_config` defaults to `false` (the board runs its own config:
/// the "stack" default). A manual [`Default`] impl makes that intent explicit
/// rather than relying on `bool`'s derive default (see `spec/HOST_RULES.md` §9).
///
/// ```toml
/// [host]
/// disable_firmware_config = false   # global default: false = stack (board runs), true = replace
/// ```
#[derive(Debug, Deserialize)]
pub struct HostDefaults {
    /// Global default for whether the board runs its own config (`false` = stack)
    /// or is replaced by the host layer (`true`). Per-rule
    /// `disable_firmware_config: Option<bool>` (on [`LayerRule`]/[`CallbackRule`])
    /// overrides this; `None` inherits this value.
    #[serde(default)]
    pub disable_firmware_config: bool, // default false (stack)
}

impl Default for HostDefaults {
    fn default() -> Self {
        Self {
            disable_firmware_config: false,
        }
    }
}

/// A `[[layer_rules]]` entry — maps a window [`Pattern`] to a host layer number.
///
/// Layer rules are evaluated **first-match-wins**; the active host layer must be
/// `>= 224` (see `spec/HOST_RULES.md` §3 C8). `pattern` (TOML key `match`) and
/// `layer` are **required** — a `[[layer_rules]]` missing either is a
/// deserialization error (strictness for the future `--validate-rules`).
///
/// ```toml
/// [[layer_rules]]
/// match = "alacritty"                       # class-only pattern (Pattern::Single)
/// layer = 224
/// disable_firmware_config = true           # optional override (default inherits [host])
///
/// [[layer_rules]]
/// match = ["*chrome*", "*youtube*"]         # [class_pattern, title_pattern] (== WT(), Pattern::Parts)
/// layer = 225
/// case_sensitive = false                    # optional, default false
/// ```
#[derive(Debug, Deserialize)]
pub struct LayerRule {
    /// Window pattern (TOML key `match`). A bare string → [`Pattern::Single`]
    /// (class-only); a 2-element array → [`Pattern::Parts`] (class + title, == firmware `WT()`).
    #[serde(rename = "match")]
    pub pattern: Pattern,
    /// The host layer number to activate on match. Must be `>= 224` (host layer range).
    pub layer: u8,
    /// Whether the [`Pattern`] matches case-sensitively. Defaults to `false`
    /// (firmware default is case-insensitive).
    #[serde(default)]
    pub case_sensitive: bool,
    /// Per-rule override of [`HostDefaults::disable_firmware_config`]. `None`
    /// (the default when the key is absent) ⇒ inherit the `[host]` global default
    /// (resolved by P3.M1.T1.S2).
    #[serde(default)]
    pub disable_firmware_config: Option<bool>, // None => inherit [host]
}

/// A `[[callback_rules]]` entry — maps a window [`Pattern`] to a set of
/// callback names to `enable`/`disable` (by registry name).
///
/// Callback rules are evaluated **all-match** (every matching rule fires).
/// `pattern` (TOML key `match`) is **required**; `enable`/`disable` default to
/// empty vectors (a rule may only `enable`, or only `disable`).
///
/// ```toml
/// [[callback_rules]]
/// match = "neovide"
/// enable = ["vim_lazy", "disable_vim"]      # run on focus-in
/// disable = ["vim_lazy"]                    # optional: force-off override
///
/// [[callback_rules]]
/// match = ["*chrome*", "*claude*"]
/// enable = ["vim_lazy", "disable_vim"]
/// disable_firmware_config = true           # for this window, skip the string -> board can't match
/// ```
#[derive(Debug, Deserialize)]
pub struct CallbackRule {
    /// Window pattern (TOML key `match`). A bare string → [`Pattern::Single`];
    /// a 2-element array → [`Pattern::Parts`] (== firmware `WT()`).
    #[serde(rename = "match")]
    pub pattern: Pattern,
    /// Callback names to enable (run on focus-in). Defaults to empty when absent.
    #[serde(default)]
    pub enable: Vec<String>,
    /// Callback names to disable (force-off override). Defaults to empty when absent.
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
        assert_eq!(rs.host.disable_firmware_config, false);

        // layer_rules: first-match-wins ordering preserved.
        assert_eq!(rs.layer_rules.len(), 2);

        // layer_rules[0]: class-only Single + explicit override.
        assert_eq!(
            rs.layer_rules[0].pattern,
            Pattern::Single("alacritty".into())
        );
        assert_eq!(rs.layer_rules[0].layer, 224);
        assert_eq!(rs.layer_rules[0].case_sensitive, false);
        assert_eq!(rs.layer_rules[0].disable_firmware_config, Some(true));

        // layer_rules[1]: class+title Parts + no override (inherits [host]).
        assert_eq!(
            rs.layer_rules[1].pattern,
            Pattern::Parts("*chrome*".into(), "*youtube*".into())
        );
        assert_eq!(rs.layer_rules[1].layer, 225);
        assert_eq!(rs.layer_rules[1].case_sensitive, false);
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
        assert_eq!(rs.callback_rules[0].case_sensitive, false);
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
        assert_eq!(rs.host.disable_firmware_config, false);
        assert_eq!(rs.layer_rules.len(), 1);
    }

    #[test]
    fn test_rules_empty_toml_is_all_default() {
        let rs: RuleSet = toml::from_str("").unwrap();
        assert_eq!(rs.host.disable_firmware_config, false);
        assert!(rs.layer_rules.is_empty());
        assert!(rs.callback_rules.is_empty());
        // Equivalent to RuleSet::default().
        assert_eq!(rs.host.disable_firmware_config, RuleSet::default().host.disable_firmware_config);
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
        assert_eq!(rs.host.disable_firmware_config, false);
        assert!(rs.layer_rules.is_empty());
        assert!(rs.callback_rules.is_empty());
    }
}