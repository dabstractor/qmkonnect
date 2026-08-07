# cargo-generate-rpm — authoritative config syntax (from upstream README)

Source: fetched the upstream README verbatim from
`https://raw.githubusercontent.com/cat-in-136/cargo-generate-rpm/master/README.md`
(270 lines). Verified 2026-08-07. This is the single source of truth for the
`.rpm` recipe; it OVERRIDES `spec/PACKAGING.md §4.4` wherever they differ.

## CRITICAL: spec §4.4 has INVALID TOML — corrected here

spec/PACKAGING.md §4.4 prints:

```toml
require-local = { "hidapi" >= "0.10", "libxdo", "zenity", "libnotify", "systemd" }
```

This is **invalid TOML** (`>=` is not a valid inline-table operator) AND
`require-local` is **not a real field**. The upstream-correct forms are:

- **Field name is `requires` (plural) / a sub-table `[…generate-rpm.requires]`** — never `require` or `require-local`.
- **Versioned requires use a MAP in a dedicated sub-table**, value = version-constraint STRING:

```toml
[package.metadata.generate-rpm.requires]
hidapi    = ">= 0.10"
libxdo    = "*"
zenity    = "*"
libnotify = "*"
systemd   = "*"
```

- A **space is mandatory** between the operator and the version: `">= 0.10"`, NOT `">=0.10"`.
- `package = "*"` means any version. `package = "version"` (bare) is REJECTED — use `package = "= version"` for an exact pin.

## `[package.metadata.generate-rpm]` fields (verbatim from README)

- `name`, `version`, `license`, `summary` — fall back to `package.{name,version,license,description}` if absent.
- `url` — falls back to `package.homepage` then `package.repository`. **Our Cargo.toml has NEITHER → `url` MUST be set explicitly.**
- `assets` — **mandatory**, array of inline tables.
  - `source` (relative to project root; `*` wildcard allowed), `dest` (full install path, or dir ending `/`), `mode` (octal string e.g. `"755"`).
  - Optional per-asset: `config` (bool / `"noreplace"` / `"missingok"` / array), `doc` (bool), `user`, `group`, `caps`.
- `release` — optional string; included in the output filename (`-<release>`).
- `epoch` — optional number.
- `vendor` — optional string.
- **Scriptlets** (each: `optional string or file path`): `pre_install_script`, `pre_uninstall_script`, `pre_trans_script`, `pre_untrans_script`, `post_install_script`, `post_uninstall_script`, `post_trans_script`, `post_untrans_script`. Each has `*_script_flags` (int) + `*_script_prog` (string array). A **file-path value is read and its contents embedded** as the `%post`/`%postun` scriptlet.
- `requires` — inline list form (e.g. `requires = ["pkg"]`). For versioned requires, use the **sub-table** above.
- `auto-req` — `"no"`/`"disabled"` disables auto-dependency detection. **Default = auto** (builtin ldd-based, or `/usr/lib/rpm/find-requires` if present). Keep default — it adds correct `libfoo.so.N()` requires automatically.
- `require-sh` — bool; `/bin/sh` is ALWAYS added unless `false`. **Keep default true** (our postin/postun are shell scripts).
- `obsoletes`, `conflicts`, `provides`, `recommends`, `supplements`, `suggests`, `enhances` — optional dependency lists / sub-tables.

## Build mechanics (verbatim)

```
Upon run `cargo generate-rpm` on your cargo project, a binary RPM package file
will be created in `target/generate-rpm/XXX.rpm`.

In advance, run `cargo build --release` and strip the debug symbols
(`strip -s target/release/XXX`), because these are not run upon `cargo generate-rpm`
as of now.
```

- **`cargo generate-rpm` does NOT build for you** — run `cargo build --release` first. Our `[profile.release] strip = true` already strips; no manual `strip` needed.
- **Output path:** `target/generate-rpm/qmkonnect-<version>-<release>.<arch>.rpm`, i.e. with `release = "1"` → `qmkonnect-0.2.8-1.x86_64.rpm`.
- It does NOT depend on `rpmbuild` (uses the `rpm` crate) → can run on any host for STRUCTURAL validation, though the AUTHORITATIVE build is Fedora CI (glibc + unified hidapi).

## LICENSE shipping

cargo-generate-rpm has **no `license-file` mechanism** (unlike cargo-deb). The
`license` field only sets the RPM `License:` tag. To ship the LICENSE text, add
it as an **explicit asset**, conventionally to `/usr/share/licenses/<name>/LICENSE`.

## auto-req interaction

The builtin auto-req adds library-level requires (`libhidapi.so.0()(64bit)`, etc.).
Our explicit `[package.metadata.generate-rpm.requires]` adds **package-level**
requires with a version floor. They coexist; keep both. Do NOT set `auto-req = "no"`.

## RPM scriptlet semantics (vs Debian hooks) — the upgrade gotcha

- `%post` (post_install_script) runs on install AND on upgrade ($1: 1=install, 2=upgrade).
  Our postin logic (instantiate service, reload udev, global enable) is safe/idempotent
  for both → no guard needed.
- `%postun` (post_uninstall_script) runs after erase AND after upgrade ($1: 1=erase, 2=upgrade).
  On an upgrade, `%postun` runs for the OLD package right after the NEW one is installed —
  so unguarded teardown would rip out the service + rules the new package just (re-)set up.
  ⇒ **Guard the postun cleanup with `if [ "$1" = "0" ]; then …; fi`** (erase-only). This
  matches spec §4.4's wording "reverse on erase" and is the standard RPM idiom.