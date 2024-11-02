{
  description = "Flake for manga-tui, a terminal manga reader and downloader";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib nixpkgs.legacyPackages.${system}).overrideToolchain rust;

        commonArgs = {
          src = craneLib.cleanCargoSource self;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            pkg-config
          ];
          buildInputs = with pkgs; [
            openssl
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in
      {
        packages = rec {
          manga-tui = craneLib.buildPackage (
            commonArgs
            // {
              inherit cargoArtifacts;
            }
          );

          default = manga-tui;
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            git
            openssl
            pkg-config
          ];
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
