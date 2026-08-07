# Research Notes — P1.M1.T2.S2: udev rule + hid-id + systemd integration in the Nix package

Repo: **QMKonnect** (`/home/dustin/projects/qmkonnect`). This is the second half of
the Nix flake: S1 shipped the **binaries** (`flake.nix` + `packaging/nix/README.md`,
both now on disk); S2 (this) wires the **udev rule + hid-id helper + systemd user
service INTO the package** (a `postInstall` phase) and adds a **NixOS module**
(`nixosModules.default`) so NixOS users get one-line enablement. It also updates
the README's Mode-A docs. Env-gated: `nix` is NOT in the authoring env — the
implementer validates in a Nix env (same gate as S1).

## 0. What S1 left on disk (verified this session — the S2 starting state)

- `flake.nix` (repo root): matches S1's PRP verbatim. `outputs = { self, nixpkgs,
  flake-utils, ... }: flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ]
  (system: { packages.default; packages.qmkonnect (buildRustPackage, cargoHash =
  pkgs.lib.fakeHash, the 9 buildInputs, RUSTFLAGS=hidrawFlag, doCheck=false, meta);
  devShells.default; })`. **No `postInstall`; no `nixosModules`.** `cargoHash` is
  still `fakeHash` (S1's Nix-env iteration not yet run — irrelevant to S2's
  structure; the implementer resolves it in the Nix env).
- `packaging/nix/README.md`: S1's content (install + manual post-install +
  `nix develop` + hidapi note). It ends with: "(A future update will ship the rule
  + helper + template from the Nix package itself.)" — **that future update IS S2.**
- `src/bin/hid_id.rs` → `[[bin]] name = "qmkonnect-hid-id"` (confirmed in
  `Cargo.toml`). `qmkonnect` (src/main.rs) + `qmkonnect-hid-id` both build into
  `$out/bin` automatically.

## 1. The two hardcoded paths that MUST be rewritten to Nix store paths (the crux)

NixOS has **no FHS** — there is no `/usr/lib/udev/` and no `/usr/bin/`. Binaries +
helpers live in the Nix store (`/nix/store/<hash>-qmkonnect-<ver>/...`). Two
shipped files hardcode FHS paths and would break verbatim on NixOS:

### (a) The udev rule (`packaging/linux/udev/69-qmkonnect-rawhid.rules`)
```
SUBSYSTEM=="hidraw", IMPORT{program}="/usr/lib/udev/qmkonnect-hid-id %S%p"
```
`IMPORT{program}` runs the helper by absolute path. On NixOS `/usr/lib/udev/...`
does not exist → the rule's IMPORT silently fails → `ID_QMKONNECT` is never set →
no device is ever tagged → QMKonnect never gets permissions. **Fix:** in
`postInstall`, copy the rule to `$out/lib/udev/rules.d/` then `substituteInPlace`
`/usr/lib/udev/qmkonnect-hid-id` → `$out/lib/udev/qmkonnect-hid-id` (the package's
own store path, where the helper is also installed). udev then runs the helper at
its real store path.

### (b) The systemd template (`packaging/linux/systemd/qmkonnect.service.template`)
```
ExecStart=/usr/bin/qmkonnect
```
Same problem: `/usr/bin/qmkonnect` doesn't exist on NixOS → the service fails to
start. **Fix:** in `postInstall`, `substitute` the template to
`$out/lib/systemd/user/qmkonnect.service` (instantiating `.template` → `.service`)
with `/usr/bin/qmkonnect` → `$out/bin/qmkonnect`.

`BindsTo=dev-qmkonnect_device.device` needs NO rewrite — it names the device unit
derived from the `qmkonnect_device` **symlink** the (rewritten) udev rule creates;
that symlink name is FHS-independent and works on NixOS. `ReadWritePaths=%t`
(`%t` = `$XDG_RUNTIME_DIR`) and the rest are FHS-independent too.

## 2. The `postInstall` (canonical Nixpkgs idiom — `install` + `substitute`)

`buildRustPackage` runs `postInstall` in a stdenv that provides `install`,
`substitute`, `substituteInPlace` as setup hooks. `$out/bin/{qmkonnect,
qmkonnect-hid-id}` already exist (buildRustPackage installs all `[[bin]]` targets
before postInstall). `${./packaging/...}` is a flake path literal → its store path
(evaluated at flake-eval time, independent of `src = ./.`). The three steps:

```nix
postInstall = ''
  install -Dm755 $out/bin/qmkonnect-hid-id $out/lib/udev/qmkonnect-hid-id
  install -Dm644 ${./packaging/linux/udev/69-qmkonnect-rawhid.rules} \
    $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules
  substituteInPlace $out/lib/udev/rules.d/69-qmkonnect-rawhid.rules \
    --replace "/usr/lib/udev/qmkonnect-hid-id" "$out/lib/udev/qmkonnect-hid-id"
  substitute ${./packaging/linux/systemd/qmkonnect.service.template} \
    $out/lib/systemd/user/qmkonnect.service \
    --replace "/usr/bin/qmkonnect" "$out/bin/qmkonnect"
'';
```
- `install -D` creates parent dirs; `-m755` (executable helper) / `-m644` (rule).
- `substituteInPlace` edits in place (used after `install` of the rule).
- `substitute` copies + replaces in one step (used to instantiate the service).
- The helper is COPIED (parity with the Arch PKGBUILD, which `install -Dm755`s it;
  it's a tiny pure-std binary, so the duplication vs a symlink is negligible and
  avoids any udev IMPORT symlink-resolution edge case).

## 3. The NixOS module (`nixosModules.default`) — the three things the contract asks

`nixosModules` is a **system-agnostic** flake output (a single module applied to
whatever NixOS host imports it). It must be a SIBLING of the `eachSystem` block in
`outputs`, NOT inside it. So `outputs` becomes:
```nix
outputs = { self, nixpkgs, flake-utils, ... }:
  let perSystem = flake-utils.lib.eachSystem [ ... ] (system: { ... }); in
  perSystem // { nixosModules.default = { config, lib, pkgs, ... }: { ... }; };
```
The module references the flake's own package via `self.packages.${pkgs.system}
.qmkonnect` (`self` is captured by the closure; `pkgs.system` is the host system).
This is the canonical flake-ships-package-AND-module pattern.

The three contract requirements + the NixOS-native way to meet each:

**(1) Register the udev rule via `services.udev.packages`.**
```nix
services.udev.packages = [ self.packages.${pkgs.system}.qmkonnect ];
```
NixOS links the package's `lib/udev/rules.d/*.rules` into the system udev rules.
Because the package is now a system dependency, its store path is **realized** —
so the rule's `IMPORT{program}` (rewritten to `$out/lib/udev/qmkonnect-hid-id`)
resolves. The rule sets `GROUP="input" MODE="0660" TAG+="uaccess"`, creates the
`qmkonnect_device` symlink, and sets `ENV{SYSTEMD_USER_WANTS}+=qmkonnect.service`.

**(2) Enable the systemd user service.**
```nix
systemd.packages = [ self.packages.${pkgs.system}.qmkonnect ];
```
NixOS's `systemd.packages` links the package's `lib/systemd/user/*.service` into
the **system-wide user unit library** (`/etc/systemd/user/`) — making the unit
*available* to every user's user-manager (NixOS wiki "Systemd/User Services":
"User unit files are installed in /etc/systemd/user/. This is kind of a 'library'
of available user services."). "Enabling" then happens two ways, both supported:
  - **Auto-start on plug:** the udev rule's `ENV{SYSTEMD_USER_WANTS}+=
    qmkonnect.service` makes systemd start the user service when the device
    appears — no manual `systemctl` needed for hotplug autostart.
  - **Login-time start (optional):** the user runs `systemctl --user enable --now
    qmkonnect.service` once (the [Install] `WantedBy=default.target` honors it).
NixOS base has NO declarative `systemd.user.services.<name>` option (that's
home-manager), so "make available + rely on SYSTEMD_USER_WANTS / manual enable"
is the correct base-NixOS ceiling. The README documents both.

**(3) Add the user to the `input` group.**
The udev rule sets `GROUP="input"` (group-accessible node) **+** `TAG+="uaccess"`
(per-session ACL via systemd-logind). PRD §2: "`uaccess` is **primary**; the
`GROUP`/`MODE` fallback is required because `uaccess` … can race logind on
replug." So on a normal NixOS desktop with logind, `uaccess` suffices and `input`
membership is the **fallback**. The contract still says "add the user to input",
so the module exposes an **optional** `services.qmkonnect.user` option:
```nix
users.users = lib.optionalAttrs (cfg.user != null) {
  ${cfg.user}.extraGroups = [ "input" ];
};
```
(`lib.optionalAttrs` only emits the attr when `user` is set, so the default
`null` adds nobody. Setting `user = "alice"` augments alice's groups via the
module system's attrset merge — requires alice to be defined in `users.users`
elsewhere in the config, which is the user's responsibility.)

`options.services.qmkonnect = { enable = mkEnableOption …; user = mkOption …; }`
gates all of (1)/(2)/(3) behind `lib.mkIf cfg.enable`.

## 4. Why `enable` gates everything (the module discipline)

A NixOS module that **unconditionally** added a udev package + systemd package to
every host that imports it would force-build qmkonnect on machines that don't want
it. `mkIf cfg.enable` makes it opt-in: `services.qmkonnect.enable = true` in
`configuration.nix` is the one-line enablement. This is the universal NixOS module
convention (every `services.*` module does this).

## 5. The README update (Mode A, rides with the work)

S1's README has a "Post-install: udev rule + systemd service (one-time, manual)"
section + the "(A future update will ship the rule + helper + template from the
Nix package itself.)" note. S2:
- **Adds a "NixOS" section** (the recommended one-line path): add
  `inputs.qmkonnect.nixosModules.default` to `imports`, set
  `services.qmkonnect.enable = true` (+ optional `services.qmkonnect.user`).
- **Relabels the existing manual section "Non-NixOS (Nix on another distro)"** —
  those users `nix profile install` the binary then do the manual udev/systemd
  steps (unchanged from S1, but now framed as the non-NixOS fallback). Keep the
  cross-ref to `docs/installation.md` + `spec/LINUX.md §6`.
- **Replaces** the "(A future update…)" sentence with "The package now ships the
  rule + helper + service; the NixOS module wires them automatically."

## 6. Scope boundaries + the env gate

- **DO** modify `flake.nix` (add `postInstall` to `packages.qmkonnect`; restructure
  `outputs` to `let perSystem = … in perSystem // { nixosModules.default = …; }`).
- **DO** modify `packaging/nix/README.md` (add NixOS section + relabel manual).
- **DO NOT** modify `Cargo.toml`, `src/`, the udev rule / service template SOURCES,
  `docs/`, or the Arch PKGBUILD. The substitution happens at BUILD time in
  postInstall — the source files stay FHS-pathed (they're correct for Arch/Ubuntu).
- **ENV GATE.** `nix` is not installed in the authoring env. The implementer runs
  `nix build .#qmkonnect` (postInstall runs; verify `result/lib/udev/…`,
  `result/lib/systemd/user/…`), `nix flake check` (evals `nixosModules.default`),
  and (ideally) a `nixos-rebuild build-vm --flake .#test` smoke test in a Nix env.
- **cargoHash is still fakeHash** (S1's iteration not yet run in a Nix env). S2's
  validation REQUIRES a successful `nix build`, which itself requires the real
  cargoHash. So S2's runbook includes the cargoHash iteration (inherited from S1)
  as a precondition — the postInstall only runs once the build succeeds.

## 7. Validation (env-gated; keyed to clear Nix output)

- `nix flake check` → passes (evals the `nixosModules.default` output too — catches
  Nix-expr errors in the module).
- `nix build .#qmkonnect` → succeeds (after the cargoHash iteration). Then:
  - `ls result/lib/udev/qmkonnect-hid-id result/lib/udev/rules.d/69-qmkonnect-rawhid.rules result/lib/systemd/user/qmkonnect.service` → all 3 present.
  - `grep -c '/usr/lib/udev/qmkonnect-hid-id' result/lib/udev/rules.d/69-qmkonnect-rawhid.rules` → **0** (rewritten to the store path); `grep store result/lib/udev/rules.d/*.rules | grep qmkonnect-hid-id` → the `/nix/store/.../qmkonnect-hid-id` path.
  - `grep ExecStart result/lib/systemd/user/qmkonnect.service` → `/nix/store/.../bin/qmkonnect` (NOT `/usr/bin/qmkonnect`).
- (Optional, deeper) `nix run nixpkgs#nixos-rebuild -- build-vm --flake .#test`
  with a minimal `nixosConfigurations.test` that imports `nixosModules.default` +
  sets `services.qmkonnect.enable = true` → builds a bootable VM (proves the module
  evals + integrates without circular refs). Heavy; `nix flake check` is the cheap
  gate.