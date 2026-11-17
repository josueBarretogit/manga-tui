# Home Manager module for manga-tui, exposed by the flake as `homeManagerModules.default`
#
# Generates the config file described in README.md / src/config.rs. Every key is always written
# (with the same defaults as manga-tui) so manga-tui never has to rewrite the read-only file
# Home Manager links into place.
self:
{
  config,
  lib,
  pkgs,
  ...
}:
let
  inherit (lib) mkOption types;

  cfg = config.programs.manga-tui;

  tomlFormat = pkgs.formats.toml { };

  configFile = tomlFormat.generate "manga-tui-config.toml" cfg.settings;

  # manga-tui resolves its config directory with `directories::ProjectDirs::from("", "", "manga-tui")`
  isDarwin = pkgs.stdenv.hostPlatform.isDarwin;
in
{
  options.programs.manga-tui = {
    enable = lib.mkEnableOption "manga-tui, a terminal manga reader and downloader";

    package = mkOption {
      type = types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.default;
      defaultText = lib.literalExpression "manga-tui.packages.\${pkgs.stdenv.hostPlatform.system}.default";
      description = "The manga-tui package to install.";
    };

    settings = mkOption {
      type = types.submodule {
        freeformType = tomlFormat.type;

        options = {
          download_type = mkOption {
            type = types.enum [
              "cbz"
              "raw"
              "epub"
              "pdf"
            ];
            default = "cbz";
            description = "The format of the manga downloaded.";
          };

          image_quality = mkOption {
            type = types.enum [
              "low"
              "high"
            ];
            default = "low";
            description = ''
              Download image quality, low quality means images are compressed and is recommended for slow
              internet connections.
            '';
          };

          amount_pages = mkOption {
            type = types.ints.u8;
            default = 5;
            description = "Pages around the currently selected page to try to prefetch.";
          };

          auto_bookmark = mkOption {
            type = types.bool;
            default = true;
            description = ''
              Whether or not bookmarking is done automatically, if false you decide which chapter to bookmark.
            '';
          };

          track_reading_when_download = mkOption {
            type = types.bool;
            default = false;
            description = "Whether or not downloading a manga counts as reading it on services like anilist.";
          };

          check_new_updates = mkOption {
            type = types.bool;
            default = true;
            description = "Enable / disable checking for new updates.";
          };

          default_manga_provider = mkOption {
            type = types.enum [
              "mangadex"
              "weebcentral"
              "mangapill"
            ];
            default = "mangadex";
            description = ''
              Which manga provider is used when running manga-tui, it can be overridden with the `-p` flag.
            '';
          };

          track_reading_history = mkOption {
            type = types.bool;
            default = true;
            description = "Enable / disable tracking reading history with services like anilist.";
          };

          anilist = mkOption {
            type = types.submodule {
              freeformType = tomlFormat.type;

              options = {
                client_id = mkOption {
                  type = types.str;
                  default = "";
                  description = "Your client id from your anilist account, leave it empty to not read it from the config file.";
                };

                access_token = mkOption {
                  type = types.str;
                  default = "";
                  description = ''
                    Your access token from your anilist account, leave it empty to not read it from the config file.

                    The value ends up world-readable in the Nix store, prefer running `manga-tui anilist init`
                    which stores the credentials in the system keyring.
                  '';
                };
              };
            };
            default = { };
            description = "Anilist-related config.";
          };
        };
      };
      default = { };
      example = lib.literalExpression ''
        {
          download_type = "epub";
          image_quality = "high";
          default_manga_provider = "weebcentral";
          check_new_updates = false;
        }
      '';
      description = ''
        Configuration written to manga-tui's {file}`config.toml`, see the configuration section of the
        manga-tui README for the meaning of each key.
      '';
    };
  };

  config = lib.mkIf cfg.enable (
    lib.mkMerge [
      {
        home.packages = [ cfg.package ];

        warnings = lib.optional (cfg.settings.anilist.access_token != "") ''
          programs.manga-tui.settings.anilist.access_token is set, it will be stored world-readable in the Nix store.
          Leave it empty and run `manga-tui anilist init` to store the anilist credentials in the system keyring instead.
        '';
      }

      (lib.mkIf (!isDarwin) {
        xdg.configFile."manga-tui/config.toml".source = configFile;
      })

      (lib.mkIf isDarwin {
        home.file."Library/Application Support/manga-tui/config.toml".source = configFile;
      })
    ]
  );
}
