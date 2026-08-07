name: "P1.M1.T2.S3 — Update Nix flake postInstall to ship XDG autostart .desktop (F17)"
description: "Add a 4th step to flake.nix's postInstall that copies packaging/linux/xdg/qmkonnect.desktop into the package's $out/etc/xdg/autostart/ (the universal Linux login-autostart fallback, F17), and note it in packaging/nix/README.md. Eval-only Nix validation (cargoHash placeholder)."

---

## Goal

**Feature Goal**: Ship the XDG autostart `.desktop` entry inside the Nix package
produced by `flake.nix`, so that login-autostart works on non-NixOS / manual Nix
installs (the F17 universal fallback) alongside the existing udev rule, hid-id
helper, and systemd user service. This brings the Nix package into parity with the
Arch/AUR/.deb/.rpm packages, which all already install
`/etc/xdg/autostart/qmkonnect.desktop` (cross-package contract,
`spec/PACKAGING.md` §4.7).

**Deliverable**:
1. An added **4th step** in `flake.nix`'s `postInstall` that copies
   `packaging/linux/xdg/qmkonnect.desktop` → `$out/etc/xdg/autostart/qmkonnect.desktop`.
2. A short note in `packaging/nix/README.md` that the package now ships the XDG
   autostart `.desktop`.

**Success Definition**:
- `nix flake check --no-build` passes (evaluates every flake output for BOTH
  `x86_64-linux` and `aarch64-linux` without instantiating a build).
- The new `install` step is syntactically inside the existing
  `postInstall = '' … ''` heredoc block, numbered `# 4.` to match steps `# 1.`–`# 3.`.
- The destination path and Nix store-source reference spellings exactly match the
  cross-package contract: `$out/etc/xdg/autostart/qmkonnect.desktop` and
  `${./packaging/linux/xdg/qmkonnect.desktop}`.
- `packaging/nix/README.md` mentions the `.desktop` ships in the package.
- No changes to flake inputs, `flake.lock`, `cargoHash`, the NixOS module, or any
  Rust source.

## Why

- **F17 (universal Linux autostart)** requires the XDG autostart `.desktop` to ship
  in *every* Linux package. The Nix flake (`P1.M1.T2.S2`, Complete) currently ships
  the binary, hid-id helper, udev rule, and systemd template — but not the `.desktop`.
  `spec/LINUX.md` §6.3: "every Linux package ships an XDG autostart entry".
- On **NixOS** the systemd user service is the *primary* autostart (the module's
  `systemd.packages` + the udev rule's `SYSTEMD_USER_WANTS`); the `.desktop` is
  belt-and-suspenders there.
- On **non-NixOS / manual Nix** (`nix profile install` on Arch/Ubuntu/Fedora/…),
  there is no systemd unit wired up for the user, so the `.desktop` is the
  login-autostart path. Shipping it in the package makes it available there.
- This is the last gap to bring the Nix package to feature-parity with the other
  Linux channels for F17.

## What

A single additive `install -Dm644` line (plus a descriptive comment) appended to
`flake.nix`'s existing `postInstall` heredoc, and a README note. **No other file
changes.** The `.desktop` is copied **verbatim** (no `substitute`/`substituteInPlace`
path rewriting), because its `Exec=qmkonnect` is an intentional bare PATH lookup,
not a hardcoded `/usr/...` absolute path (see Gotchas).

### Success Criteria

- [ ] `flake.nix` `postInstall` has a `# 4.` step copying the `.desktop` to
      `$out/etc/xdg/autostart/qmkonnect.desktop` (mode 644).
- [ ] The source file is referenced as `${./packaging/linux/xdg/qmkonnect.desktop}`
      (Nix store-copy interpolation, matching steps 2 & 3).
- [ ] `packaging/nix/README.md` notes the `.desktop` ships in the package (F17).
- [ ] `nix flake check --no-build` passes (both arches, eval-only).
- [ ] `flake.lock` is NOT created or modified (inputs unchanged).
- [ ] No substitution of `Exec=qmkonnect` (bare name kept, per §4.7).

## All Needed Context

### Context Completeness Check

> "If someone knew nothing about this codebase, would they have everything needed
> to implement this successfully?" — YES. The exact file, the exact destination
> path, the exact Nix idiom, the exact validation command, and the three load-bearing
> gotchas (eval-only validation, no flake.lock, no Exec substitution) are all below.

### Documentation & References

```yaml
# MUST READ before editing
- url: (in-repo) spec/PACKAGING.md §4.5 (Nix flake) + §4.7 (XDG autostart .desktop)
  why: "§4.7 is the source of truth for the cross-package .desktop contract:
        /etc/xdg/autostart/qmkonnect.desktop, mode 644, and explicitly states
        'Exec=qmkonnect relies on /usr/bin (or the Nix store path) being on PATH
        in the session … The Nix module and the .desktop are independent'."
  critical: "Confirms Exec=qmkonnect (bare) is INTENTIONAL — do NOT substitute it
             to a Nix store path; that would break the cross-package parity and
             contradict §4.7. Confirms dest path + mode for the Nix step."

- url: (in-repo) spec/LINUX.md §6.3 (XDG autostart .desktop — universal fallback F17)
  why: "Explains why the .desktop ships in every Linux package: 'every Linux package
        ships an XDG autostart entry at /etc/xdg/autostart/qmkonnect.desktop'."
  critical: "On systemd distros the service is primary; the .desktop is redundant
             but harmless (single-instance owned by the tray/runner, not launcher)."

# EXACT pattern to copy (existing flake.nix postInstall steps 2 & 3)
- file: flake.nix
  why: "Steps 2 & 3 show the exact idiom: install a source file into the package
        via install -Dm644 ${./packaging/linux/<...>} $out/<...>, optionally followed
        by substitute/substituteInPlace. Step 4 is the same idiom with NO substitute."
  pattern: |
            # 4. <comment>
            install -Dm644 ${./packaging/linux/xdg/qmkonnect.desktop} \
              $out/etc/xdg/autostart/qmkonnect.desktop
  gotcha: "The `${./...}` interpolation copies the source file into the Nix store at
           eval time. Must use the relative `./packaging/linux/xdg/...` path (same
           root the flake lives in). The step MUST be inside the postInstall = '' … ''
           heredoc, before its closing ''."

# CROSS-PACKAGE CONSISTENCY — the other packages already do exactly this
- file: packaging/linux/aur/PKGBUILD
  why: "Line 68 is the AUR (P1.M1.T1.S3) equivalent of this task:
        install -Dm644 \"${stage}/xdg/qmkonnect.desktop\"
          \"${pkgdir}/etc/xdg/autostart/qmkonnect.desktop\". Same path, same mode."
  pattern: "Static .desktop copied verbatim to /etc/xdg/autostart/qmkonnect.desktop."

- file: packaging/linux/arch/PKGBUILD
  why: "Line 51 is the Arch source-PKGBUILD equivalent:
        install -Dm644 \"../xdg/qmkonnect.desktop\" \"$pkgdir/etc/xdg/autostart/qmkonnect.desktop\"."
  pattern: "Confirms the identical dest path + mode across all Linux channels."

# THE FILE BEING SHIPPED (already created by P2.M6.T1.S1, Complete)
- file: packaging/linux/xdg/qmkonnect.desktop
  why: "The static XDG autostart entry to copy. NoDisplay=true (autostart-only,
        not in app menus), Exec=qmkonnect (bare), Icon=input-keyboard."
  gotcha: "Do NOT edit this file in this task — it is a shared input owned by
           P2.M6.T1.S1. This task only references it."

# DOCS target
- file: packaging/nix/README.md
  why: "The flake README (Mode A docs deliverable). Intro paragraph currently lists
        'static udev rule + systemd user service' as what the package ships — add
        the XDG autostart .desktop to that list and/or a one-line note."
  pattern: "Match the existing README's tone (terse, code-fenced commands)."

# VALIDATION — the CI Nix job
- url: (in-repo) .github/workflows/release.yml (the `nix:` job, ~lines 302–346)
  why: "Defines the ONLY valid CI-style gate: `nix flake check --no-build`. The job
        comment explains WHY --no-build is load-bearing (cargoHash = pkgs.lib.fakeHash
        placeholder) and that flake.lock is absent by design (each run re-resolves)."
  critical: "Do NOT add a `nix build .#qmkonnect` gate — it FAILS on the fake hash
             by design (a separate tracked follow-up). Do NOT generate flake.lock."
```

### Current Codebase tree (relevant slice)

```bash
flake.nix                          # EDIT — add postInstall step # 4
packaging/nix/README.md            # EDIT — note the .desktop ships
packaging/linux/xdg/
└── qmkonnect.desktop              # INPUT (owned by P2.M6.T1.S1; do not edit)
packaging/linux/aur/PKGBUILD       # REFERENCE pattern (already ships .desktop)
packaging/linux/arch/PKGBUILD      # REFERENCE pattern (already ships .desktop)
spec/PACKAGING.md  §4.5, §4.7      # REFERENCE — cross-package contract
spec/LINUX.md      §6.3            # REFERENCE — F17 rationale
.github/workflows/release.yml      # REFERENCE — nix flake check --no-build gate
# NOTE: flake.lock does NOT exist and must NOT be created here (by design).
```

### Desired Codebase tree (changes only)

```bash
flake.nix                  # postInstall gains step # 4 (install .desktop)
packaging/nix/README.md    # one paragraph/line notes the .desktop ships
# (no new files; no flake.lock; no source changes)
```

### Known Gotchas of our codebase & Nix quirks

```bash
# CRITICAL: Validation is EVAL-ONLY. The flake ships with
#   cargoHash = pkgs.lib.fakeHash   (deliberate placeholder)
# because qmk-notifier is a git dependency and Cargo.lock carries no vendor hash.
# A real `nix build .#qmkonnect` FAILS with a fixed-output hash mismatch until a
# human runs the one-time hash-iteration. That is a SEPARATE tracked task — NOT
# this one. The ONLY valid CI-style gate is:
#     nix flake check --no-build
# (evaluates every output for both x86_64-linux + aarch64-linux, no build).

# CRITICAL: flake.lock does NOT exist and must NOT be created here. Inputs
# (nixpkgs, flake-utils) are unchanged, so the contract's "regenerate flake.lock
# if inputs change" conditional is FALSE → do nothing. CI re-resolves each run by
# design (access-tokens is additive). Do NOT run `nix flake lock` / `nix flake update`.

# CRITICAL: Do NOT substitute `Exec=qmkonnect`. Steps 2 & 3 rewrite paths because
# those files contain HARDCODED `/usr/...` absolute paths that don't exist in Nix.
# The .desktop has a BARE PATH-lookup name (no absolute path), which is INTENTIONAL
# per spec/PACKAGING.md §4.7 ("relies on /usr/bin (or the Nix store path) being on
# PATH in the session"). Rewriting it to a store path would break cross-package
# parity (.deb/.rpm/AUR all ship Exec=qmkonnect) and contradict the spec.
#   - NixOS module already puts the binary on PATH via environment.systemPackages.
#   - `nix profile install` puts it at ~/.nix-profile/bin (on PATH for a normal setup).

# The NixOS module (nixosModule in flake.nix) needs NO changes — it already lists
# the package in environment.systemPackages / services.udev.packages / systemd.packages.
# On NixOS systemd is PRIMARY autostart; the .desktop is belt-and-suspenders.

# The `${./packaging/linux/xdg/qmkonnect.desktop}` Nix string interpolation COPIES
# the source file into the Nix store at EVAL time (same mechanism steps 2 & 3 use
# for the udev rule + systemd template). This is correct; do not use `substitute`
# for the source argument.

# postInstall is a heredoc: postInstall = '' … ''. The new step MUST be placed
# inside that block (before the closing ''). Use the project's `# N.` comment style.
```

## Implementation Blueprint

### The exact edit to `flake.nix`

The existing `postInstall` ends after the systemd-service step (currently step `# 3.`).
Append a new step `# 4.` immediately after it, inside the same `'' … ''` block:

```nix
            postInstall = ''
              # 1. udev helper — the rule IMPORTs it by absolute path.
              install -Dm755 $out/bin/qmkonnect-hid-id $out/lib/udev/qmkonnect-hid-id

              # 2. static udev rule — rewrite the hardcoded helper path to the
              #    Nix store path so udev can run it at device-add time.
              install -Dm644 ${./packaging/linux/udev/69-qmkonnect-rawhid.rules} \
                $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules
              substituteInPlace $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules \
                --replace "/usr/lib/udev/qmkonnect-hid-id" "$out/lib/udev/qmkonnect-hid-id"

              # 3. systemd user service — instantiate .template -> .service and
              #    rewrite the hardcoded ExecStart to the Nix store path.
              substitute ${./packaging/linux/systemd/qmkonnect.service.template} \
                $out/lib/systemd/user/qmkonnect.service \
                --replace "/usr/bin/qmkonnect" "$out/bin/qmkonnect"

              # 4. XDG autostart .desktop — the universal Linux login-autostart
              #    fallback (F17; spec/LINUX.md §6.3, spec/PACKAGING.md §4.7).
              #    Static file copied verbatim (NoDisplay=true hides it from app
              #    menus). Exec=qmkonnect is a BARE PATH lookup — intentionally
              #    NOT substituted to a store path (the module's
              #    environment.systemPackages on NixOS, or `nix profile install`
              #    on other distros, puts the binary on PATH), matching the
              #    cross-package contract (.deb/.rpm/AUR all ship Exec=qmkonnect).
              #    Belt-and-suspenders: NixOS uses systemd (primary); this is for
              #    non-NixOS / manual Nix installs.
              install -Dm644 ${./packaging/linux/xdg/qmkonnect.desktop} \
                $out/etc/xdg/autostart/qmkonnect.desktop
            '';
```

(The block above is shown in full for context; the ONLY new lines are the
`# 4.` comment + the two-line `install -Dm644 …` command. Do not duplicate steps
1–3 — they already exist.)

### The README edit (`packaging/nix/README.md`)

Two minimal, in-tone touch-ups (Mode A: note that the `.desktop` is included):

1. In the intro paragraph, extend the "what the package ships" sentence. Current:
   > "…and also ships the static udev rule + systemd user service from the package
   > (rewritten to the Nix store path) plus a `nixosModules.default` NixOS module…"

   Change to also name the XDG autostart `.desktop`, e.g.:
   > "…and also ships the static udev rule, systemd user service, **and XDG autostart
   > `.desktop`** from the package (rewritten to the Nix store path where they carry
   > hardcoded `/usr` paths) plus a `nixosModules.default` NixOS module…"

2. Optionally add one line under the **"Non-NixOS (Nix on another distro)"** section
   noting the package now includes `etc/xdg/autostart/qmkonnect.desktop`, so a user
   who wants login-autostart on a non-systemd distro can symlink it from the profile
   to `~/.config/autostart/` (the host DE reads `/etc/xdg/autostart/` and
   `~/.config/autostart/`, not the Nix profile's `etc/xdg/autostart/`). Keep it terse.

Keep the existing tone (terse, code-fenced commands, links to `spec/LINUX.md §6` /
`docs/installation.md`). Do not restructure the README.

### Implementation Tasks (ordered by dependencies)

```yaml
Task 1: EDIT flake.nix — append postInstall step # 4
  - ADD: a `# 4.` comment block + `install -Dm644 ${./packaging/linux/xdg/qmkonnect.desktop} $out/etc/xdg/autostart/qmkonnect.desktop`
  - FOLLOW pattern: flake.nix postInstall steps # 2 and # 3 (the `${./...}` store-copy
    idiom + the `# N.` numbered-comment style). Step 4 has NO substitute (unlike 2/3)
    because the .desktop has no hardcoded /usr path.
  - PLACEMENT: INSIDE the existing postInstall = '' … '' heredoc, immediately after
    the `# 3.` systemd-service step, before the closing ''.
  - NAMING: dest path EXACTLY `$out/etc/xdg/autostart/qmkonnect.desktop` (cross-package contract).

Task 2: EDIT packaging/nix/README.md — note the .desktop ships
  - ADD: extend the intro "what ships" sentence to include the XDG autostart .desktop.
  - OPTIONAL ADD: one terse line under "Non-NixOS" about symlinking the shipped
    .desktop to ~/.config/autostart for login-autostart on non-systemd distros.
  - FOLLOW pattern: the README's existing tone (terse, code-fenced, links to specs).
  - DO NOT restructure the README or change install commands.

Task 3: VALIDATE — eval-only
  - RUN: nix flake check --no-build   (primary gate; both arches, eval-only)
  - FALLBACK (if nix not installed in the dev env): nix-instantiate --parse flake.nix
    (syntax check) + visual confirmation the step is inside the heredoc.
  - DO NOT run `nix build .#qmkonnect` (fails on cargoHash=fakeHash by design).
  - DO NOT run `nix flake lock` / `nix flake update` (inputs unchanged; flake.lock
    is absent by design).
```

### Integration Points

```yaml
NIX PACKAGE OUTPUT:
  - add to: flake.nix → packages.qmkonnect → postInstall (append step # 4)
  - effect: $out/etc/xdg/autostart/qmkonnect.desktop ships in the built profile/system env

NIXOS MODULE:
  - NO CHANGE. environment.systemPackages already = [ pkg ], so qmkonnect is on PATH
    (resolving Exec=qmkonnect). systemd remains the primary autostart; .desktop is
    belt-and-suspenders.

FLAKE INPUTS / flake.lock:
  - NO CHANGE. nixpkgs + flake-utils unchanged; flake.lock stays absent (CI re-resolves).

DOCS:
  - packaging/nix/README.md (this task, Mode A)
  - spec/PACKAGING.md §4.5 already describes the flake; §4.7 already documents the
    .desktop contract — no spec edits needed for this 1-point leaf task.
```

## Validation Loop

### Level 1: Syntax & Style (Immediate Feedback)

```bash
# Primary eval gate — evaluates EVERY flake output for BOTH x86_64-linux and
# aarch64-linux WITHOUT instantiating a build. This is the exact CI command
# (.github/workflows/release.yml `nix:` job). Must pass.
nix flake check --no-build
# Expected: builds nothing, exits 0. A failure here means a Nix syntax/eval error
# in your edit — read the message (it will name the line).

# Fallback if nix is not installed in your dev env (syntax-only sanity check):
nix-instantiate --parse flake.nix >/dev/null && echo "flake.nix parses OK"
# Expected: "flake.nix parses OK". (This only checks lexer/parser health, not
# that the new install step is well-placed — visually confirm placement too.)

# Grep-verify the exact dest path + source-ref spelling landed correctly:
grep -n 'xdg/autostart/qmkonnect.desktop' flake.nix
grep -n '${./packaging/linux/xdg/qmkonnect.desktop}' flake.nix
# Expected: both match the line you added inside postInstall.
```

> **DO NOT** run `nix build .#qmkonnect` — it FAILS with a fixed-output hash
> mismatch because `cargoHash = pkgs.lib.fakeHash` is a deliberate placeholder
> (qmk-notifier git dep has no vendor hash in Cargo.lock). Resolving that is a
> separate tracked follow-up, NOT this task. `nix flake check --no-build` is the
> correct gate and stays GREEN today.
>
> **DO NOT** run `nix flake lock` / `nix flake update` — inputs are unchanged and
> `flake.lock` is absent by design (CI re-resolves nixpkgs + flake-utils each run).

### Level 2: Structural Verification (the heredoc placement)

```bash
# Confirm the new step is INSIDE postInstall (between its opening '' and the next ''),
# not accidentally placed in the surrounding Nix attrset:
awk '/postInstall = '"'"''"'"'/{f=1} f{print} /^            '"'"';$/{if(f)exit}' flake.nix | grep -nE 'install -Dm644 .*\$\{./packaging/linux/xdg'
# Expected: prints the new install line with a line number. (Simpler: just open
# flake.nix and confirm step # 4 sits after # 3 and before the closing '' of postInstall.)

# Confirm you did NOT touch the source .desktop (shared input, owned by P2.M6.T1.S1):
git status --short packaging/linux/xdg/qmkonnect.desktop
# Expected: no output (unmodified).

# Confirm you did NOT create flake.lock:
git status --short flake.lock
# Expected: no output (flake.lock must remain absent).
```

### Level 3: Cross-Channel Parity (consistency, no build needed)

```bash
# Confirm the Nix dest path matches every other Linux channel's dest path:
grep -rn 'etc/xdg/autostart/qmkonnect.desktop' \
  flake.nix packaging/linux/aur/PKGBUILD packaging/linux/arch/PKGBUILD spec/PACKAGING.md
# Expected: the SAME relative dest path appears in all channels (Nix uses
# $out/etc/xdg/autostart/...; PKGBUILDs use .../etc/xdg/autostart/...; spec lists
# /etc/xdg/autostart/qmkonnect.desktop). This proves cross-package parity.

# Confirm the README note landed:
grep -ni 'xdg autostart\|\.desktop' packaging/nix/README.md
# Expected: at least one match mentioning the .desktop ships in the package.
```

### Level 4: (N/A for this task)

There is no runtime/build integration test for this leaf task — the package cannot
be built locally (cargoHash placeholder) and the `.desktop` is a static data file.
Eval parity + cross-channel path consistency + README note is the complete
validation surface. Do not invent a build gate.

## Final Validation Checklist

### Technical Validation

- [ ] `nix flake check --no-build` passes (both `x86_64-linux` + `aarch64-linux`, eval-only).
- [ ] (Fallback) `nix-instantiate --parse flake.nix` parses OK, if nix flake unavailable.
- [ ] New `# 4.` step is inside the `postInstall = '' … ''` heredoc.
- [ ] Dest path is exactly `$out/etc/xdg/autostart/qmkonnect.desktop`.
- [ ] Source ref is exactly `${./packaging/linux/xdg/qmkonnect.desktop}`.
- [ ] `flake.lock` was NOT created/modified.
- [ ] `cargoHash` and flake `inputs` were NOT touched.
- [ ] `packaging/linux/xdg/qmkonnect.desktop` (shared input) was NOT modified.

### Feature Validation

- [ ] All success criteria from "What" met.
- [ ] `packaging/nix/README.md` notes the `.desktop` ships in the package.
- [ ] Dest path matches the other Linux channels (AUR/Arch/.deb/.rpm) + spec §4.7.
- [ ] No `substitute` of `Exec=qmkonnect` (bare PATH lookup kept per §4.7).
- [ ] NixOS module left unchanged (already wires PATH + systemd; .desktop is belt-and-suspenders).

### Code Quality & Docs

- [ ] New comment uses the project's `# N.` numbered-step style and references F17 / §6.3 / §4.7.
- [ ] README edit matches the existing terse tone; no restructure.
- [ ] No new dependencies, no env vars, no Cargo changes.

---

## Anti-Patterns to Avoid

- ❌ Don't run `nix build .#qmkonnect` as a gate — it fails on the deliberate
  `cargoHash = pkgs.lib.fakeHash` placeholder. Use `nix flake check --no-build`.
- ❌ Don't create or regenerate `flake.lock` — inputs are unchanged and the lock is
  absent by design (CI re-resolves each run).
- ❌ Don't `substitute` / rewrite `Exec=qmkonnect` — it is an intentional bare PATH
  lookup (spec §4.7); steps 2 & 3 only rewrite files that carry hardcoded `/usr/...`
  paths. Rewriting it breaks cross-package parity.
- ❌ Don't edit the NixOS module — it already wires `environment.systemPackages`
  (PATH), `services.udev.packages`, and `systemd.packages`. Systemd is primary on
  NixOS; the `.desktop` is belt-and-suspenders.
- ❌ Don't edit the shared input `packaging/linux/xdg/qmkonnect.desktop` (owned by
  P2.M6.T1.S1) — this task only references it.
- ❌ Don't add a new file or restructure the flake/README — this is a 1-point
  additive edit + a one-line doc note.

---

## Confidence Score

**9 / 10** — One-pass success is highly likely. The task is a small, well-bounded
additive edit following an exact existing idiom (steps 2 & 3), with a clearly
defined cross-package contract, an unambiguous eval-only validation gate, and three
explicitly-documented landmines (eval-only / no flake.lock / no Exec substitution)
that account for the −1. No unknowns remain; all inputs (`.desktop`, `flake.nix`)
already exist and are Complete.