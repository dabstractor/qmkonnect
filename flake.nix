{
  description = "Cross-platform window activity notifier for QMK keyboards";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    let
      # Per-system outputs (the package + the dev shell) — from S1 (P1.M1.T2.S1).
      # nixosModules (below) is system-agnostic, so it sits OUTSIDE eachSystem and
      # is merged in with `//` at the end.
      perSystem = flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
        let
          pkgs = import nixpkgs { inherit system; };

          # Build-time deps shared by the package + the dev shell.
          nativeBuildInputs = [ pkgs.pkg-config ];
          buildInputs = with pkgs; [
            gtk3
            glib
            libayatana-appindicator
            libx11
            libxcb
            xdotool       # provides libxdo.so for the `libxdo` crate
            hidapi
            libusb
            systemd       # libudev (hidapi hidraw) + geteuid()
          ];

          # Parity with the Arch PKGBUILD: link the hidraw backend so usage/
          # usage_page device matching works (NOT the libusb backend). If the first
          # `nix build` fails with "cannot find -lhidapi-hidraw" your Nixpkgs ships
          # the UNIFIED hidapi (>=0.14) — see packaging/nix/README.md "hidapi link"
          # note + the fallback (drop the flag; unified libhidapi auto-selects
          # hidraw on Linux, so matching still works).
          hidrawFlag = "-C link-arg=-lhidapi-hidraw";
        in
        {
          packages.default = self.packages.${system}.qmkonnect;

          packages.qmkonnect = pkgs.rustPlatform.buildRustPackage {
            pname = "qmkonnect";
            # version is derived from Cargo.toml (buildRustPackage reads it).
            src = ./.;

            # STEP 1 of the cargoHash iteration: start with the fake hash, run
            # `nix build .#qmkonnect`, read the "got: sha256-…" from the failure,
            # paste it here, rebuild. (Required because Cargo.lock does not carry
            # the hash of the qmk-notifier GIT dependency.)
            cargoHash = pkgs.lib.fakeHash;

            inherit nativeBuildInputs buildInputs;
            RUSTFLAGS = hidrawFlag;

            # The bin's tests need --test-threads=1 (shared global debouncer) +
            # HID hardware; non-deterministic in a Nix build. CI runs them.
            doCheck = false;

            # Ship the udev rule + hid-id helper + systemd user service ALONGSIDE
            # the two binaries. NixOS has no FHS, so the rule's hardcoded
            # /usr/lib/udev/qmkonnect-hid-id and the service's /usr/bin/qmkonnect
            # are rewritten to this package's Nix store path ($out/...). The
            # NixOS module (nixosModules.default) registers the rule + service;
            # non-NixOS users do the manual install (see packaging/nix/README.md).
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
              #    Static file copied VERBATIM (NoDisplay=true hides it from app
              #    menus). Exec=qmkonnect is a BARE PATH lookup, intentionally
              #    NOT substituted to a store path: the module's
              #    environment.systemPackages (NixOS) or `nix profile install`
              #    (other distros) puts the binary on PATH, matching the
              #    cross-package contract (.deb/.rpm/AUR all ship Exec=qmkonnect).
              #    Belt-and-suspenders: NixOS uses systemd (primary); this is for
              #    non-NixOS / manual Nix installs.
              install -Dm644 ${./packaging/linux/xdg/qmkonnect.desktop} \
                $out/etc/xdg/autostart/qmkonnect.desktop
            '';

            meta = with pkgs.lib; {
              description = "Cross-platform window activity notifier for QMK keyboards";
              homepage = "https://github.com/dabstractor/qmkonnect";
              license = licenses.mit;
              mainProgram = "qmkonnect";
              platforms = [ "x86_64-linux" "aarch64-linux" ];
            };
          };

          devShells.default = pkgs.mkShell {
            inherit nativeBuildInputs buildInputs;
            packages = with pkgs; [ cargo rustc clippy rustfmt ];
            RUSTFLAGS = hidrawFlag;
          };
        });

      # The NixOS module — system-agnostic. Reference's this flake's own package
      # via self.packages.${pkgs.system}.qmkonnect (the postInstall above means it
      # ships the rule + helper + service). Import via
      # inputs.qmkonnect.nixosModules.default in a NixOS configuration.nix; enable
      # with services.qmkonnect.enable = true.
      nixosModule = { config, lib, pkgs, ... }:
        let
          cfg = config.services.qmkonnect;
          pkg = self.packages.${pkgs.system}.qmkonnect;
        in
        {
          options.services.qmkonnect = {
            enable = lib.mkEnableOption (lib.mdDoc "QMKonnect — udev rule + systemd user service for QMK keyboards");

            user = lib.mkOption {
              type = lib.types.nullOr lib.types.str;
              default = null;
              description = lib.mdDoc ''
                Username to add to the `input` group (the udev rule's GROUP="input"
                fallback). Optional: the rule's `uaccess` tag grants the active
                logged-in user access via systemd-logind, so this is only needed
                if you rely on the raw group permission instead of the session
                ACL. The named user must be defined elsewhere in your configuration.
              '';
            };
          };

          config = lib.mkIf cfg.enable {
            # Put the qmkonnect binary on PATH (for manual runs / the tray /
            # `qmkonnect --list-callbacks`). The systemd service uses its
            # store-path ExecStart, so it doesn't depend on PATH, but the
            # user-facing binary should be runnable when the module is enabled.
            environment.systemPackages = [ pkg ];

            # 1. Static udev rule: tags QMK Raw HID devices, sets GROUP/MODE +
            #    uaccess, creates the qmkonnect_device symlink, and
            #    SYSTEMD_USER_WANTS the service on plug. The rule's IMPORT runs the
            #    helper at its (postInstall-rewritten) Nix store path.
            services.udev.packages = [ pkg ];

            # 2. systemd user service: makes the unit available in the system-wide
            #    user unit library (/etc/systemd/user/). The udev rule's
            #    SYSTEMD_USER_WANTS auto-starts it on device plug; for login-time
            #    start the user runs `systemctl --user enable --now qmkonnect`.
            systemd.packages = [ pkg ];

            # 3. Optional: add a user to the `input` group (the GROUP fallback;
            #    uaccess is primary, so this is opt-in).
            users.users = lib.optionalAttrs (cfg.user != null) {
              ${cfg.user}.extraGroups = [ "input" ];
            };
          };
        };
    in
    perSystem // {
      nixosModules.default = nixosModule;
    };
}