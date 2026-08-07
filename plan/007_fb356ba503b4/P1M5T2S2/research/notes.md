# Research Notes — P1.M5.T2.S2

**Item:** Add Nix flake check + asdf plugin test + asdf publish CI steps.
**Two files edited:** `.github/workflows/ci.yml` (+`nix-check` job) and
`.github/workflows/release.yml` (+`asdf-plugin` job, test-then-publish). **Mode A** inline
secret doc for `ASDF_PLUGIN_DEPLOY_KEY` (ride-along, no docs/*.md file).

---

## 1. The LOAD-BEARING correction: fakeHash → `nix flake check --no-build`

`flake.nix` (P1.M1.T2.S2, Complete) ships with:

```nix
cargoHash = pkgs.lib.fakeHash;   # sha256-AAAAAAAA… (deliberate placeholder)
```

Its own comment: *"STEP 1 of the cargoHash iteration: start with the fake hash, run
`nix build .#qmkonnect`, read the 'got: sha256-…' from the failure, paste it here, rebuild.
(Required because Cargo.lock does not carry the hash of the qmk-notifier GIT dependency.)"*

**Consequence:** a BUILDING `nix flake check` (or `nix build .#qmkonnect`) FAILS today with a
cargo vendor hash mismatch — the real hash was never captured. The contract's literal "runs
`nix flake check`" would be a RED gate on every push to main.

**Decision (load-bearing):** the `nix-check` job runs **`nix flake check --no-build`** (eval-only).
- `--no-build` evaluates every flake output (packages, devShells, nixosModules) WITHOUT
  instantiating the build — so `fakeHash` (a valid string) is never fetched/verified → eval passes.
- It still catches real flake breakage: Nix syntax/type errors, missing/broken inputs, a
  malformed `let`, a `packages.default` that fails to construct, a devShell that won't build its
  derivation object.
- Verified semantics: Nix 2.18/2.22 manual — "`nix flake check --no-build` … verifies the flake
  can be evaluated … `--no-build`: Do not build checks." The flake-parts issue #367 confirms
  `--no-build` still EVALUATES (a real eval failure surfaces there) — it only skips the build.

**Follow-up (out of scope, documented):** once the one-time `cargoHash` iteration is done
(modify `flake.nix` — separate task), flip to a full `nix flake check` + `nix build .#qmkonnect
--no-link`. Resolving cargoHash + adding a `checks.*.nixos-module-eval` output are both
`flake.nix` edits → explicitly OUT OF SCOPE (contract OUTPUT = only ci.yml + release.yml).

**Known limitation of `--no-build`:** it does NOT fully instantiate the `nixosModules.default`
against a NixOS config (it only type-checks it's a module-shaped value). Full module eval would
need a `checks.x86_64-linux.*` output added to the flake — out of scope. Documented honestly.

---

## 2. install-nix-action: flakes enabled by default + pin @v31

- **Flakes auto-enabled:** the action's README + multiple sources confirm *"The experimental
  `flakes` and `nix-command` features are enabled."* (disable by overriding `experimental-features`
  in `extra_nix_config`). → **NO** `experimental-features` config needed to run `nix flake check`.
- **Version:** `git ls-remote --tags https://github.com/cachix/install-nix-action.git` → latest
  major is **v31** (v31.11.0 newest patch). Repo convention is MAJOR pins
  (`actions/checkout@v4`, `softprops/action-gh-release@v2`, `vedantmgoyal9/winget-releaser@v2`,
  `webfactory/ssh-agent@v0.9.0`) → pin **`cachix/install-nix-action@v31`**.
- **`access-tokens` best practice:** `extra_nix_config: access-tokens = github.com=${{ secrets.GITHUB_TOKEN }}`
  lifts the GitHub API rate limit when nix resolves flake inputs. **flake.lock is ABSENT**
  (verified: no flake.lock in repo), so each `nix flake check` run re-resolves nixpkgs +
  flake-utils — a token prevents intermittent 403/rate-limit failures. Uses the default
  GITHUB_TOKEN (no new secret). ci.yml has no top-level `permissions:` block (default read token
  suffices).

---

## 3. asdf publish model = homebrew-tap / scoop-bucket deploy-key pattern (NOT AUR, NOT Winget)

`packaging/asdf/publish.sh` (P1.M4.T1.S2, Complete) header + `REMOTE` default:
```
REMOTE="${ASDF_QMKONNECT_REMOTE:-git@github.com:dabstractor/asdf-qmkonnect.git}"
```
- **SSH push** to the plugin repo `dabstractor/asdf-qmkonnect` (external repo, WE own it).
- **Deploy key model = homebrew-tap/scoop-bucket:** a per-REPO GitHub deploy key (public half on
  `dabstractor/asdf-qmkonnect` Settings → Deploy keys, write access; private half = the
  `ASDF_PLUGIN_DEPLOY_KEY` Actions secret). Loaded via `webfactory/ssh-agent@v0.9.0` — the SAME
  pin both sibling jobs use (release.yml lines 439 + 518). Confirmed in publish.sh's header:
  *"store the PRIVATE half as the ASDF_PLUGIN_DEPLOY_KEY Actions secret."*
- **NOT** the AUR model (per-account AUR SSH key) and **NOT** the Winget model (classic PAT to an
  external repo we DON'T own). The secret name `ASDF_PLUGIN_DEPLOY_KEY` is fixed by the contract +
  publish.sh.
- **publish.sh sets its OWN git identity** (step 5: `git config user.email/name` if unset) — UNLIKE
  homebrew-tap/scoop-bucket where the JOB sets git identity. So the asdf job does NOT set git config.
- **publish.sh rejects a leading 'v'** (the `case "$VERSION" in v*)` guard) → pass the BARE version
  (`${GITHUB_REF_NAME#v}`), same idiom as all build-less sibling jobs.

---

## 4. asdf plugin TEST (no secrets): shellcheck + publish.sh --dry-run

Two dependency-free test steps, both run BEFORE the deploy key (need no secret):

**(a) shellcheck** — `shellcheck packaging/asdf/bin/* packaging/asdf/lib/*.bash packaging/asdf/publish.sh`.
- **Verified CLEAN** on this dev box (shellcheck 0.11.0) — a hard gate is SAFE (no pre-existing warnings).
- The plugin scripts carry `# shellcheck disable=SC1091` for the sourced `utils.bash` (intentional;
  not a real failure). shellcheck is **preinstalled on ubuntu-latest** GitHub runners.

**(b) publish.sh --dry-run against a LOCAL mock remote** — the robust end-to-end test:
```bash
git init --bare /tmp/asdf-mock.git
ASDF_QMKONNECT_REMOTE=file:///tmp/asdf-mock.git ./packaging/asdf/publish.sh --dry-run "$VERSION"
```
- `--dry-run` exercises the ENTIRE flow (clone mock → copy bin/lib/metadata → chmod +x bin/* →
  sed-stamp version → git add → verify exec bit 100755 → commit) and skips only the push.
- Uses `file://` → no SSH, no secret. Validates publish.sh logic + the plugin file inventory on
  EVERY release before pushing to the real repo.
- **Why NOT `asdf-vm/actions/plugin-test`:** that installs a real version from GitHub Releases
  (needs the just-published release + udev/systemd for the Linux binary, which won't work in CI).
  shellcheck + dry-run is the pragmatic, dependency-free equivalent that actually validates the
  plugin we ship.

---

## 5. Job placement / coordination

- **ci.yml** current jobs: `fmt`, `build-and-test` (2 jobs). → append `nix-check` at END (3 jobs).
  ci.yml triggers: `push: branches: [main]` + `workflow_dispatch`. nix-check runs on every main push.
- **release.yml** current jobs (after parallel P1.M5.T2.S1 lands): macos, windows, linux-binary,
  arch, publish, aur, homebrew-tap, scoop-bucket, **winget** (9 jobs; winget confirmed present on
  disk as the LAST job). → append `asdf-plugin` at END (10 jobs).
- **Parallel coordination:** P1.M5.T2.S1 (Winget) appends `winget` after scoop-bucket. This task
  appends `asdf-plugin` after `winget`. Zero overlap (different jobs, different secrets, different
  deploy-key repos). If winget is NOT yet present at implementation time, append at the very END of
  release.yml regardless — do not insert between jobs.
- All publish jobs share the tag-only gate `needs: [publish]` + `if: github.event_name == 'push'`.

---

## 6. Validation reality on the Linux dev box

- **Nix is NOT installed** here (`nix --version` → not found). So the `nix-check` job's real eval
  is a DEFERRED CI gate (mirrors the winget PRP's deferred real-PR gate). Local validation for
  ci.yml = YAML parse + actionlint + grep invariants.
- **shellcheck IS installed** (0.11.0) → the asdf test's shellcheck half CAN be validated locally
  (already confirmed clean).
- **publish.sh dry-run CAN be validated locally** (no secret, file:// mock). Recommend the
  implementer run it once locally to confirm the flow.
- Both edited files: `python3 -c "import yaml; yaml.safe_load(...)"` + actionlint + grep gates.

---

## 7. Version source / inputs consumed (no source-code edits)

- **Nix input:** `flake.nix` (P1.M1.T2.S2) — consumed READ-ONLY (the job never edits it).
- **asdf input:** `packaging/asdf/publish.sh` + `bin/{list-all,download,install}` + `lib/utils.bash`
  + `README.md` + `.tool-versions` + `mise.toml` + `CHANGELOG.md` (all P1.M4.T1.S1/S2, Complete) —
  consumed by the job/publish.sh, never edited.
- **Version:** bare, from `${GITHUB_REF_NAME#v}` (the `v*` tag push). cargo-release cuts the tag from
  Cargo.toml → transitively the Cargo.toml version (external_deps.md "Version Source of Truth"). The
  nix-check job needs NO version (it evaluates the in-repo flake as-is).