{
  description = "Flake for manga-tui, a terminal manga reader and downloader";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
    # only used by the checks testing the Home Manager module
    home-manager = {
      url = "github:nix-community/home-manager";
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
      home-manager,
    }:
    {
      homeManagerModules = rec {
        manga-tui = import ./nix/home-manager.nix self;
        default = manga-tui;
      };
    }
    // flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };
        inherit (pkgs.lib) cleanSource;

        rust = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        craneLib = (crane.mkLib nixpkgs.legacyPackages.${system}).overrideToolchain rust;

        commonArgs = {
          # src = craneLib.cleanCargoSource self;
          #
          # use regular nixpkgs lib.cleanSource so that `public` directory
          # isn't removed, causing build failure
          src = cleanSource self;
          strictDeps = true;
          nativeBuildInputs = with pkgs; [
            pkg-config
            perl # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
            openssl.dev # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
          ];
          buildInputs = with pkgs; [
            dbus
            openssl # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
            perl
            cacert # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
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

        checks = import ./nix/tests.nix {
          inherit pkgs home-manager;
          module = self.homeManagerModules.default;
          package = self.packages.${system}.manga-tui;
        };

        devShells.default = craneLib.devShell {
          packages = with pkgs; [
            git
            openssl
            openssl.dev # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
            dbus
            pkg-config
            perl
            cacert # added due to failing workflow: https://github.com/josueBarretogit/manga-tui/actions/runs/13379018167/job/37364084916?pr=114
          ];
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
