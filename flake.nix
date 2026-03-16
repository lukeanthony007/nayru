{
  description = "Nayru — voice-enabled text-to-speech server";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          config.allowUnfree = true;
          overlays = [ (import rust-overlay) ];
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "clippy"
            "rust-analyzer"
            "rust-src"
            "rustfmt"
          ];
        };

        tauriDeps = with pkgs; [
          webkitgtk_4_1
          gtk3
          libsoup_3
          glib-networking
          libappindicator-gtk3
          librsvg
          cairo
          pango
          atk
          gdk-pixbuf
        ];
      in {
        devShells.default = pkgs.mkShell {
          packages = with pkgs; [
            rustToolchain
            llvmPackages_latest.clang
            llvmPackages_latest.lld
            pkg-config
            openssl
            cmake

            # Node / Bun
            bun
            nodejs_22

            # Audio (cpal/rodio)
            alsa-lib

            # Dev tools
            just
            jq
            fd
            ripgrep
          ] ++ tauriDeps;

          LIBCLANG_PATH = "${pkgs.llvmPackages_latest.libclang.lib}/lib";
          RUST_SRC_PATH = "${rustToolchain}/lib/rustlib/src/rust/library";
          GIO_MODULE_PATH = "${pkgs.glib-networking}/lib/gio/modules";

          shellHook = ''
            export PATH="$PWD/node_modules/.bin:$PATH"
          '';
        };
      }
    );
}
