# Research Notes — P1.M2.T1.S1: Homebrew Cask formula for QMKonnect.app DMG

**Repo**: QMKonnect (`/home/dustin/projects/qmkonnect`). **Pure packaging** (no Rust/source change).
**New files**: `packaging/homebrew/Casks/qmkonnect.rb` (the cask) + `packaging/homebrew/README.md` (Mode A docs).
**Scope boundary**: this is the **cask formula + docs ONLY**. The tap-repo structure + CI publication
automation are the **sibling** P1.M2.T1.S2 — do NOT create tap scaffolding or CI jobs here.

---

## Parallel-context check (no conflict)

P1.M1.T2.S2 (in parallel) edits `flake.nix` + `packaging/nix/README.md` — Linux/Nix, no overlap.
The AUR channel (P1.M1.T1.S1/S2, Complete) lives in `packaging/linux/aur/` — read-only precedent for doc style.

---

## Release artifact (verified in `.github/workflows/release.yml`)

- `release.yml:82-84`: the macOS job builds `QMKonnect.dmg` then renames to
  **`QMKonnect-${version}-macos.dmg`** (version from `cargo metadata`, NO `v` prefix in metadata).
- The release **tag** is `v{version}` (cargo-release convention; URL pattern uses `releases/download/v{version}/`).
- **Universal binary**: `MACOS_UNIVERSAL=1` (release.yml:65) → `lipo` of aarch64 + x86_64 (build.sh:7-18).
  ⇒ NO `depends_on arch:` needed in the cask (one DMG serves both Apple Silicon + Intel).
- **Notarization is OPTIONAL**: release.yml:76-80 runs `xcrun notarytool submit` + `stapler staple` ONLY when
  notarization secrets are set. Default = ad-hoc signed (`codesign --sign -`, build.sh:68). ⇒ the tap-distributed
  DMG may be **unnotarized** → Gatekeeper blocks first launch ⇒ must be documented (see caveats / README).
- Bundle id: `io.mulletware.qmkonnect` (build.sh:42-44 / Info.plist CFBundleIdentifier).
- macOS per-user path (verified `src/platforms/macos.rs:469/492`, `uninstall.sh:26`):
  **`~/Library/Application Support/QMKonnect/`** (config.toml + rules.toml live here). LSUIElement menu-bar app,
  no UserDefaults/plist/CoreData ⇒ zap is minimal.

## Contract inputs confirmed
- DMG URL: `https://github.com/dabstractor/qmkonnect/releases/download/v{version}/QMKonnect-{version}-macos.dmg` ✓
- SHA256: computed by CI at release time (`shasum -a 256` of the DMG) — placeholder in the template, CI patches.
- architecture/external_deps.md §2 (Homebrew): Cask DSL `version/sha256/url/livecheck/app`, caveats for Screen
  Recording, `livecheck … strategy :header`; "Custom tap until notarized for official cask"; "Push cask file to
  homebrew tap repo on tag".

## Authoritative Homebrew references (external research)

- **Cask Cookbook** — `https://docs.brew.sh/Cask-Cookbook` — stanza order (enforced by `brew audit --cask --strict`
  via rubocop-cask `StanzaOrder`), required stanzas, `sha256 :no_check` special value, `zap trash:`/`rmdir:`,
  `caveats` heredoc, `verified:` for GitHub URLs.
- **Brew Livecheck** — `https://docs.brew.sh/Brew-Livecheck` — `strategy :header` follows the redirect and matches
  the `Location` header against a regex; GitHub `/releases/latest` 302→`/releases/tag/vX.Y.Z`.
- **Adding Software to Homebrew** — `https://docs.brew.sh/Adding-Software-to-Homebrew` — cask token rules, required stanzas.

## Stanza order (rubocop-cask StanzaOrder cop — the `--strict` gate)

version → sha256 → url → name → desc → homepage → livecheck → (depends_on/conflicts_with — omit) →
artifacts (`app`) → uninstall (omit) → **zap** → **caveats**. (Final group is `[zap, caveats]`: zap BEFORE caveats.)

## Two contract hardenings (improvements that prevent real failures)

1. **Add an explicit `regex(...)` to the livecheck.** The contract specifies only `strategy :header` against
   `/releases/latest`. The `:header` default version extraction is not guaranteed to strip the leading `v` from
   `v0.2.8`, which would make `brew livecheck` report `v0.2.8` ≠ cask `version "0.2.8"` (audit fails). The
   documented, robust idiom is:
   ```ruby
   livecheck do
     url "https://github.com/dabstractor/qmkonnect/releases/latest"
     regex(/^v?(\d+(?:\.\d+)+)$/i)   # /releases/latest → /releases/tag/v0.2.8 → captures "0.2.8"
     strategy :header
   end
   ```
2. **Add the Gatekeeper/no-quarantine note** (Mode A) to caveats + README. The released DMG is ad-hoc signed
   unless notarization secrets are set, so macOS quarantines it and Gatekeeper blocks first launch. The user must
   `brew install --cask --no-quarantine qmkonnect` OR `xattr -dr com.apple.quarantine /Applications/QMKonnect.app`
   (or right-click → Open). The contract's caveats (Screen Recording + `--show-window-info`) are KEPT; this is added.

## sha256 placeholder choice

Use **`sha256 :no_check`** (the documented Cask Cookbook special value) as the template placeholder — it's the
cleanest CI-replaceable target (`sed`/regex `^sha256 :no_check$` → `sha256 "<realhash>"`). It's honest ("unverified
until CI fills it") and lets `brew audit --cask` (non-strict) check the DSL structure. `--strict` audit + the real
hash run in the **tap repo** after CI substitutes the hash (the tap never ships `:no_check`). Document this clearly.

## Env-gated validation

`brew` is not guaranteed on the authoring box (Linux; Homebrew-on-Linux is possible but may be absent). Local
validation = `ruby -c qmkonnect.rb` (syntax, available everywhere) + a `brew audit --cask --new-cask ./qmkonnect.rb`
on a macOS/Linuxbrew host. The DMG checksum + `--strict` pass are only provable in the tap repo post-CI (real hash).

## File layout (this task creates exactly these two)
- `packaging/homebrew/Casks/qmkonnect.rb` — the cask (source of truth; CI patches version/sha256 → pushes to tap).
- `packaging/homebrew/README.md` — tap setup, install (incl. --no-quarantine), what-it-installs, notarization path.
(Mirrors `packaging/linux/aur/README.md` + `packaging/nix/README.md` conventions.)