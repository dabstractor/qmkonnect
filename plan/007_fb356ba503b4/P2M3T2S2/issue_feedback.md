# Implementation Issue Feedback (Attempt 1/3)

The previous implementation attempt encountered an issue that requires re-planning.

## Issue Details

Fundamental conflict in the PRP for P2.M3.T2.S2: the reference implementation calls `crate::platforms::gnome::probe_available(false)` from `src/runners/linux.rs`, but the `gnome` module is declared private (`mod gnome;` at src/platforms/mod.rs:22 and 26), so `cargo build --release` fails with E0603 'module `gnome` is private'. The PRP forbids editing `src/platforms/mod.rs` (both the 'Files NOT touched' list and the Level 3 `git diff --stat` gate mandate ONLY src/runners/linux.rs + docs/qmk-integration.md), yet the function is unreachable from outside the `platforms` parent module. All three resolution paths violate a PRP gate: (A) editing mod.rs to make `mod gnome;` → `pub(crate) mod gnome;` fails the diff-stat gate; (B) omitting the probe call fails the 'skip when extension present' Success Criterion; (C) reimplementing the zbus probe inline violates the explicit Anti-Pattern. The one-word visibility fix (Option A) is purely additive and behavior-neutral — it exposes an already-`pub(crate)` function to its intended crate-wide audience — but requires the orchestrator to relax S2's scope boundary, which is not an implementation-agent decision. PRP edits applied so far (the AtomicBool guard, helpers, fn rewrite, call-site move, and test module) are in place but the binary does not compile pending this decision. Awaiting direction on whether to proceed with Option A.

## Full Agent Output

Warning: No project session found with id 'prd-impl-P2M3T2S2'; creating a new session with that id.
`gnome_probe` at `src/platforms/linux.rs:175` is private (`fn`, not `pub`), so it's not reachable from `runners/linux.rs` either. There is genuinely no existing public path from outside the `platforms` module to `gnome::probe_available`.

This is the documented residual risk in the PRP's confidence score (9/10): the S1/S2 sequencing dependency. S1 declared `mod gnome;` private and only consumed it within `platforms`. The PRP for S2 assumes `crate::platforms::gnome::probe_available` is reachable from `runners/linux.rs`, but it is not, given S1's actual landed code.

The minimal fix that respects the spirit of the scope boundary (don't change S1's *logic*) while unblocking S2's *consumption* is a one-character visibility change to the module declaration in `src/platforms/mod.rs`: `mod gnome;` → `pub(crate) mod gnome;`. This is purely additive (exposing an already-`pub(crate)` function to the rest of the crate) and changes zero behavior. However, it **will fail** the PRP's Level 3 `git diff --stat` gate which mandates ONLY `src/runners/linux.rs` + `docs/qmk-integration.md`.

Per the Failure Protocol, when a fundamental issue with the plan is found, I must halt and produce a thorough explanation. Let me halt here.

I've encountered a **fundamental conflict** in the PRP that cannot be resolved without violating one of its own hard gates. Per the Failure Protocol, I'm halting with a thorough explanation.

---

## Problem Summary

The PRP for **P2.M3.T2.S2** wants `src/runners/linux.rs` to call:

```rust
crate::platforms::gnome::probe_available(false).is_ok()
```

But `crate::platforms::gnome` is **not reachable** from outside the `platforms` parent module, so `cargo build --release` fails with:

```
error[E0603]: module `gnome` is private
  --> src/runners/linux.rs:194:30
   |         if crate::platforms::gnome::probe_available(false).is_ok() {
   |                              ^^^^^  private module
```

### Why it fails (in plain terms)

Think of a Rust crate like a building with rooms inside rooms. The `platforms` room contains a smaller `gnome` room, and inside that `gnome` room is a tool called `probe_available`. The tool itself has a label saying "anyone in this building may use me" (`pub(crate) fn probe_available`).

**But the door to the `gnome` room is unmarked** (`mod gnome;` in `src/platforms/mod.rs` line 22 — it's declared private). So even though the tool is meant to be building-wide usable, nobody outside the `platforms` room can find the door to get to it.

The only code that currently calls `gnome::probe_available` lives **inside** the `platforms` room (`src/platforms/linux.rs` lines 176 — sibling files can see each other's private submodules). `runners/linux.rs` is in a *different* room, so it's locked out.

### The conflict in the PRP

The PRP simultaneously requires **two things that contradict each other**:

1. **The reference implementation** (lines ~620, 682, and the Success Criteria) calls `crate::platforms::gnome::probe_available(false)` from `src/runners/linux.rs` — which **cannot compile** given the current private module declaration.

2. **The scope boundary** (the "Files NOT touched" list + the Level 3 gate `git diff --stat` must show **ONLY** `src/runners/linux.rs` + `docs/qmk-integration.md`) **forbids editing `src/platforms/mod.rs`**, where the `mod gnome;` declaration lives.

I cannot satisfy both. To make the code compile, exactly one of these must change, and **all three options violate a PRP gate**:

- **Option A — Edit `src/platforms/mod.rs`**: change `mod gnome;` → `pub(crate) mod gnome;` (one word, zero logic change). This fixes compilation but **fails the Level 3 `git diff --stat` gate** and breaks the "Files NOT touched" rule.
- **Option B — Don't call `probe_available` from `runners/linux.rs`**: skip the extension-presence check entirely and fire the notification unconditionally on GNOME sessions. This **fails the Success Criterion** "skips the notification when `probe_available` returns `Ok`."
- **Option C — Reimplement the probe inline** in `runners/linux.rs` (duplicate the `zbus` name_has_owner call). This **violates the explicit Anti-Pattern** "Do NOT edit `src/platforms/gnome.rs` to add a helper — reuse S1's `probe_available`" and "do not reimplement."

### Why the PRP didn't catch this (residual risk)

The PRP's **Confidence Score: 9/10** notes the single residual risk is "the S1-land-first sequencing dependency (GOTCHA-5)." GOTCHA-5 only addresses whether `gnome.rs` *exists* — it does **not** address whether the `gnome` *module is reachable* from outside `platforms`. The PRP's Context Completeness check (item b) states the S1 contract is "`pub(crate) fn probe_available`" and assumes `pub(crate)` on the *function* is sufficient — but the *module* gate (`mod gnome;`) blocks it first, and the PRP never verified the module's visibility.

### What a human/orchestrator needs to decide

The cleanest, lowest-risk fix is **Option A**: a one-word visibility change in `src/platforms/mod.rs`:

```rust
// line 22 (and 26 — the cfg-gated declaration):
mod gnome;            →  pub(crate) mod gnome;
```

This is purely additive (exposing an already-`pub(crate)` function to its intended crate-wide audience), changes zero behavior, and matches how S1 intended the function to be consumed. It does, however, require **relaxing S2's scope boundary** to permit this one-line `src/platforms/mod.rs` change — which is an orchestrator/PRP-owner decision, not an implementation-agent one.

### Current state of my edits

I have already applied the parts of the PRP that are internally consistent and compile-safe:
- `src/runners/linux.rs`: added the `AtomicBool` import, the `GNOME_FIRST_RUN_FIRED` static, the `gnome_session()` + `consume_gnome_hint_shot()` helpers, rewrote `maybe_gnome_first_run_notify`, moved the call site to before `create_monitor`, removed the old Err-branch call, and added the 8-test `#[cfg(test)] mod tests`.
- The file **does not compile** until the `gnome` module visibility issue is resolved (Option A above) or the probe call is removed (Options B/C).

I have **not** touched `src/platforms/mod.rs`, `src/platforms/gnome.rs`, `Cargo.toml`, the docs, or any other file, because all roads to compilation cross a PRP gate I'm not authorized to relax.

**Awaiting direction** on whether to proceed with Option A (relaxing the scope boundary for the one-line `pub(crate) mod gnome;` change in `src/platforms/mod.rs`), or another resolution.

```json
{
   "result": "issue",
   "message": "Fundamental conflict in the PRP for P2.M3.T2.S2: the reference implementation calls `crate::platforms::gnome::probe_available(false)` from `src/runners/linux.rs`, but the `gnome` module is declared private (`mod gnome;` at src/platforms/mod.rs:22 and 26), so `cargo build --release` fails with E0603 'module `gnome` is private'. The PRP forbids editing `src/platforms/mod.rs` (both the 'Files NOT touched' list and the Level 3 `git diff --stat` gate mandate ONLY src/runners/linux.rs + docs/qmk-integration.md), yet the function is unreachable from outside the `platforms` parent module. All three resolution paths violate a PRP gate: (A) editing mod.rs to make `mod gnome;` → `pub(crate) mod gnome;` fails the diff-stat gate; (B) omitting the probe call fails the 'skip when extension present' Success Criterion; (C) reimplementing the zbus probe inline violates the explicit Anti-Pattern. The one-word visibility fix (Option A) is purely additive and behavior-neutral — it exposes an already-`pub(crate)` function to its intended crate-wide audience — but requires the orchestrator to relax S2's scope boundary, which is not an implementation-agent decision. PRP edits applied so far (the AtomicBool guard, helpers, fn rewrite, call-site move, and test module) are in place but the binary does not compile pending this decision. Awaiting direction on whether to proceed with Option A."
}
```

## Instructions

Review this feedback and create a revised PRP that addresses the issue.
If the issue is fundamentally impossible to resolve, output `"result": "fail"` with an explanation.
