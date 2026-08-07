# Codebase Findings — P1.M5.T1.S1 (AUR publication CI job)

> Direct source reading. Repo: `/home/dustin/projects/qmkonnect` (org `dabstractor`).
> Deliverable: ONE new `aur` job in `.github/workflows/release.yml`. No new files.

## 1. The existing release.yml (the file being edited)

`.github/workflows/release.yml` has 5 jobs today:
1. `macos` — QMKonnect.app + DMG (ad-hoc or notarized).
2. `windows` — Inno installer (.exe).
3. `linux-binary` — portable binary tarball (`qmkonnect-<ver>-linux-x86_64.tar.gz`).
4. `arch` — `.pkg.tar.zst` via makepkg in `container: archlinux:latest`.
5. `publish` — `needs: [macos, windows, linux-binary, arch]`, `if: github.event_name == 'push'`,
   creates the GitHub Release via `softprops/action-gh-release@v2`, uploads all artifacts.

**Triggers:** `push` of a `v*` tag (builds AND publishes) OR `workflow_dispatch` (builds WITHOUT
publishing). `permissions: contents: read` at the top; the `publish` job escalates to
`contents: write`.

**The new `aur` job `needs: [publish]`** — because publish.sh's `makepkg -g` (step 2) downloads
the release tarball, so the GitHub Release (and its `linux-binary` asset) must already be live.
It does NOT need `[arch]` — the AUR PKGBUILD sources the `linux-binary` tarball, not the .pkg.tar.zst.

**Idioms to mirror (verified verbatim):**
- Version extraction: `v=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[] | select(.name=="qmkonnect") | .version'); echo "version=$v" >> "$GITHUB_OUTPUT"` (used by macos/linux-binary/arch jobs).
- Arch container + makepkg-as-non-root (from the `arch` job):
  ```yaml
  container: archlinux:latest
  ...
  - run: pacman -Sy --noconfirm --needed base-devel rust cargo git pkg-config jq ...
  - run: useradd -m -G wheel builder; echo 'builder ALL=(ALL) NOPASSWD: ALL' >> /etc/sudoers
         chown -R builder "$GITHUB_WORKSPACE"
         su builder -c "makepkg --noconfirm --nodeps --skipinteg"
  ```
- Inline secret documentation: the `macos` job documents `APPLE_*` secrets inline in job comments +
  `env:` blocks. The `AUR_SSH_PRIVATE_KEY` secret doc follows the SAME idiom (Mode A ride-along).

## 2. publish.sh (the script the job invokes — from P1.M1.T1.S2, marked Complete)

`packaging/linux/aur/publish.sh` does ALL the AUR-side work. The CI job's job is to PROVIDE THE
ENVIRONMENT and invoke it. publish.sh's own steps:
1. `sed -i "s/^pkgver=.*/pkgver=${VERSION}/" PKGBUILD`
2. `makepkg -g` → refresh `sha256sums` (DOWNLOADS the release tarball — release must be live).
3. `makepkg --printsrcinfo > .SRCINFO`
4. (--dry-run skips) `git clone aur@aur.archlinux.org:qmkonnect-bin.git` → copy PKGBUILD + .SRCINFO +
   qmkonnect.install → `git commit -m "qmkonnect-bin v${VERSION}"` → `git push`.

**CRITICAL: publish.sh does NOT set a git identity.** (grep confirmed: no `config user.email`.)
Unlike the asdf publish.sh (P1.M4.T1.S2) which sets a fallback identity, the AUR publish.sh assumes
the caller has one. → **The CI job MUST configure `git config --global user.email/user.name` for the
builder user** before running publish.sh, or the AUR `git commit` fails with "Author identity unknown".

**publish.sh hardcodes the AUR remote:** `aur@aur.archlinux.org:qmkonnect-bin.git` (scp-like syntax —
equivalent to the contract's `ssh://aur@aur.archlinux.org/qmkonnect-bin.git`). The CI does NOT clone
the AUR itself; publish.sh does. The contract's step (c) "clones the AUR repo" is publish.sh's step 4.

**makepkg refuses root** (publish.sh header: "Do NOT run as root"). So publish.sh MUST run as the
unprivileged `builder` user, mirroring the `arch` job's `su builder -c` pattern.

## 3. PKGBUILD / .SRCINFO structure (what publish.sh patches)

`packaging/linux/aur/PKGBUILD`:
- `pkgver=0.2.8` (placeholder; publish.sh patches to the real version).
- `source=("https://github.com/dabstractor/qmkonnect/releases/download/v${pkgver}/qmkonnect-${pkgver}-linux-x86_64.tar.gz")`
  → makepkg -g downloads THIS (the linux-binary tarball). Release must be live. ✓ (needs: [publish]).
- `sha256sums=('...')` — refreshed by publish.sh's `makepkg -g`.
- It's a `-bin` package: NO build deps, NO makedepends; `package()` installs pre-built binaries.
  So `makepkg -g` only DOWNLOADS + checksums (no compilation) — fast.

`.SRCINFO` exists (pkgbase qmkonnect-bin, pkgver 0.2.8). publish.sh regenerates it.

## 4. AUR SSH auth model (from architecture/external_deps.md §1 + publish.sh header)

- The AUR supports **SSH-key auth ONLY** (no token/password).
- The PUBLIC half of the key must be registered with the AUR account
  (https://aur.archlinux.org → My Account → SSH Public Key).
- For CI: store the PRIVATE half as a GitHub Actions secret → load into ssh-agent (or a default
  IdentityFile) → publish.sh's `git clone`/`git push` use it.
- **Secret name: `AUR_SSH_PRIVATE_KEY`** (per the contract).
- `known_hosts`: the container has NO prior knowledge of `aur.archlinux.org`'s host key. The job
  must populate `~/.ssh/known_hosts` via `ssh-keyscan aur.archlinux.org` (or use
  `StrictHostKeyChecking accept-new`) so the git clone isn't interactive (no host-key prompt).

## 5. Container gotchas (the non-obvious part)

- `container: archlinux:latest` runs as **root** by default. makepkg refuses root → create `builder`.
- `openssh` is NOT in the archlinux base image and NOT installed by the existing `arch` job (which
  doesn't SSH anywhere). The `aur` job MUST add `openssh` to the pacman install (provides `ssh`,
  `ssh-keyscan`, `ssh-agent`).
- `actions/checkout@v4` checks out the repo as root (into `$GITHUB_WORKSPACE`). `chown -R builder`
  so the builder can write PKGBUILD/.SRCINFO (publish.sh patches them in place).
- The SSH key + known_hosts + ~/.ssh/config must live in **builder's** `$HOME` (`/home/builder/.ssh`),
  because publish.sh's `git clone` runs as builder. Setting them up as root then `chown builder` is
  the pattern.
- `$GITHUB_WORKSPACE` and `env:`-set vars are preserved across `su builder -c` (no `--login`), so the
  version can be passed as an env var. publish.sh also `cd "$SCRIPT_DIR"` internally (robust to cwd).
- An explicit `~/.ssh/config` Host block (`Host aur.archlinux.org; IdentityFile ~/.ssh/aur_key;
  IdentitiesOnly yes`) makes the key-type agnostic (ed25519 or rsa) and avoids relying on the default
  `~/.ssh/id_*` filename. ssh-agent not strictly required with a Host block, but harmless.

## 6. Scope boundaries (no conflicts)

- This task edits ONLY `.github/workflows/release.yml` (adds ONE job). No Rust, no Cargo, no
  packaging/ changes, no docs/* (the secret doc is an inline COMMENT in the job, per Mode A).
- publish.sh is INPUT (from P1.M1.T1.S2, Complete) — NOT modified by this task.
- The sibling CI jobs (Homebrew/Scoop = P1.M5.T1.S2, Winget = P1.M5.T2.S1, Nix/asdf CI = P1.M5.T2.S2)
  are SEPARATE work items; this PRP adds ONLY the `aur` job. They may later append their own jobs.
- The parallel P1.M4.T1.S2 (asdf plugin metadata) touches `packaging/asdf/` — ZERO overlap.
- Source-repo commit-back of the patched PKGBUILD/.SRCINFO is OUT OF SCOPE (needs a PAT + main-branch
  target; AUR publishing works regardless — see PRP Gotchas). Documented as a follow-up.

## 7. No external research needed beyond the in-repo brief

`architecture/external_deps.md` §1 + "CI Publishing Strategy" specifies the canonical pattern
verbatim: "store deploy keys as GitHub Actions secrets; on tag push (after GitHub Release publish),
git clone → update file → commit → push." publish.sh implements the clone→update→commit→push; this
PRP implements the secret-loading + Arch-container environment. The AUR SSH-key model is documented
in publish.sh's own header comment. No novel external API.