# Delta PRD (v0.2.4 → v0.2.8) Verification Results

## Summary

All five categories of change identified in the Delta PRD are **already implemented
and verified** in the production tree. The only residual actionable work is a
cosmetic documentation fix — one stale firmware naming reference that appears in
two files.

## Verified Claims

| Claim | Status | Evidence |
|-------|--------|----------|
| Naming swap (source/docs) | ✅ Done | All source uses `qmk_notifier::run`; docs use correct convention |
| Cargo dep `qmk-notifier` v0.3.0 | ✅ Done | `Cargo.toml:18`: exact match |
| `Pattern::Single` = app_class only | ✅ Done | `pattern.rs:1182` + linchpin test at line 2659 |
| Config path = `qmkonnect/` | ✅ Done | All platform files use `qmkonnect/` |
| CI uses Inno Setup | ✅ Done | `release.yml:120-125` runs `build.ps1`, no WiX reference |
| `cargo check` | ✅ Green | 0.16s, no errors/warnings |

## Residual Drift (Actionable)

Two files contain the stale `qmk-notifier_notify` (hyphen) where v0.2.8
convention requires `qmk_notifier_notify` (underscore):

1. **`docs/troubleshooting.md:647`** — the primary source:
   ```
   (there is no built-in `qmk-notifier_notify` callback — the firmware API is the
   ```
   This is a throwaway example of a non-existent callback name. The firmware
   module is `qmk_notifier` (underscore) per v0.2.8.

2. **`docs/llms_full.txt:2622`** — a generated concatenation that mirrors
   troubleshooting.md verbatim. This file is committed (not vendored) and must
   be regenerated after fixing the source.

## Clean Grep Results (confirmed zero hits)
- `package = "qmk_notifier"` — none (old crate declaration gone)
- `tag = "v0.2.1"` — none (old crate version gone)
- `build-installer.ps1` in `.github/` — none (CI uses Inno)
- `qmk-notifier/` as config path — none (only legitimate tree label in HOST_RULES.md:563)