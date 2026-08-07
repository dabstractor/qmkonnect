{
  description = "Cross-platform window activity notifier for QMK keyboards";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils, ... }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" ] (system:
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
        # note + the PRP fallback (drop the flag; unified libhidapi auto-selects
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

          # buildRustPackage builds BOTH [[bin]] targets (qmkonnect +
          # qmkonnect-hid-id) into $out/bin automatically. The udev rule +
          # hid-id helper placement + systemd template are wired into the
          # package in P1.M1.T2.S2; this task ships the binaries only.

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
}