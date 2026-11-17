# Checks for the Home Manager module, run with `nix flake check`
{
  pkgs,
  home-manager,
  module,
  package,
}:
let
  inherit (pkgs) lib;

  isDarwin = pkgs.stdenv.hostPlatform.isDarwin;

  homeDirectory = if isDarwin then "/Users/test" else "/home/test";

  # Where manga-tui looks for its config file, relative to the home directory
  configDir = if isDarwin then "Library/Application Support/manga-tui" else ".config/manga-tui";

  mkHome =
    extraConfig:
    home-manager.lib.homeManagerConfiguration {
      inherit pkgs;
      modules = [
        module
        {
          home = {
            inherit homeDirectory;
            username = "test";
            stateVersion = "25.05";
          };
        }
        extraConfig
      ];
    };

  defaultSettings = {
    download_type = "cbz";
    image_quality = "low";
    amount_pages = 5;
    auto_bookmark = true;
    track_reading_when_download = false;
    check_new_updates = true;
    default_manga_provider = "mangadex";
    track_reading_history = true;
    anilist = {
      client_id = "";
      access_token = "";
    };
  };

  customSettings = {
    download_type = "epub";
    image_quality = "high";
    amount_pages = 255;
    auto_bookmark = false;
    track_reading_when_download = true;
    check_new_updates = false;
    default_manga_provider = "weebcentral";
    track_reading_history = false;
  };

  defaultHome = mkHome { programs.manga-tui.enable = true; };

  customHome = mkHome {
    programs.manga-tui = {
      enable = true;
      settings = customSettings;
    };
  };

  anilistHome = mkHome {
    programs.manga-tui = {
      enable = true;
      settings.anilist = {
        client_id = "12345";
        access_token = "secret-token";
      };
    };
  };

  hasAccessTokenWarning = home: lib.any (lib.hasInfix "anilist.access_token") home.config.warnings;

  # Whether evaluating the given manga-tui settings fails, i.e. the module rejects them
  rejects =
    settings:
    let
      home = mkHome {
        programs.manga-tui = {
          enable = true;
          inherit settings;
        };
      };
    in
    !(builtins.tryEval (builtins.toJSON home.config.programs.manga-tui.settings)).success;

  # Builds the home configuration and checks the generated config.toml parses to `expected`
  checkGeneratedConfig =
    name: home: expected:
    pkgs.runCommand "manga-tui-hm-${name}"
      {
        nativeBuildInputs = [ pkgs.python3 ];
        expected = builtins.toJSON expected;
        passAsFile = [ "expected" ];
      }
      ''
        home_files=${home.activationPackage}/home-files

        test -x ${home.activationPackage}/home-path/bin/manga-tui

        python3 - "$home_files/${configDir}/config.toml" "$expectedPath" <<'EOF'
        import json
        import sys
        import tomllib

        with open(sys.argv[1], "rb") as generated_file:
            generated = tomllib.load(generated_file)

        with open(sys.argv[2]) as expected_file:
            expected = json.load(expected_file)

        if generated != expected:
            sys.exit(f"generated config.toml differs\n  generated: {generated}\n  expected:  {expected}")
        EOF

        touch $out
      '';
in
{
  hm-module-defaults =
    assert lib.assertMsg (
      !hasAccessTokenWarning defaultHome
    ) "no access_token warning expected by default";
    checkGeneratedConfig "defaults" defaultHome defaultSettings;

  hm-module-settings = checkGeneratedConfig "settings" customHome (defaultSettings // customSettings);

  hm-module-anilist =
    assert lib.assertMsg (hasAccessTokenWarning anilistHome)
      "expected a warning about anilist.access_token";
    checkGeneratedConfig "anilist" anilistHome (
      defaultSettings
      // {
        anilist = {
          client_id = "12345";
          access_token = "secret-token";
        };
      }
    );

  hm-module-disabled =
    let
      home = mkHome { programs.manga-tui.settings.download_type = "pdf"; };
    in
    pkgs.runCommand "manga-tui-hm-disabled" { } ''
      test ! -e "${home.activationPackage}/home-files/${configDir}/config.toml"
      test ! -e ${home.activationPackage}/home-path/bin/manga-tui
      touch $out
    '';

  hm-module-rejects-invalid-values =
    assert lib.assertMsg (rejects {
      download_type = "zip";
    }) "download_type = \"zip\" should be rejected";
    assert lib.assertMsg (rejects {
      image_quality = "medium";
    }) "image_quality = \"medium\" should be rejected";
    assert lib.assertMsg (rejects { amount_pages = 256; }) "amount_pages = 256 should be rejected";
    assert lib.assertMsg (rejects { amount_pages = -1; }) "amount_pages = -1 should be rejected";
    assert lib.assertMsg (rejects {
      default_manga_provider = "manganato";
    }) "default_manga_provider = \"manganato\" should be rejected";
    assert lib.assertMsg (rejects {
      auto_bookmark = "yes";
    }) "auto_bookmark = \"yes\" should be rejected";
    assert lib.assertMsg (!rejects customSettings) "valid settings should be accepted";
    pkgs.runCommand "manga-tui-hm-rejects-invalid-values" { } "touch $out";

  # manga-tui itself must start with, and parse, the read-only config Home Manager links into place
  hm-module-runtime =
    pkgs.runCommand "manga-tui-hm-runtime"
      {
        nativeBuildInputs = [ package ];
      }
      ''
        export HOME=$TMPDIR/home
        export XDG_CONFIG_HOME=$HOME/.config
        export XDG_DATA_HOME=$HOME/.local/share
        export XDG_CACHE_HOME=$HOME/.cache

        config_dir="$HOME/${configDir}"
        generated=${customHome.activationPackage}/home-files/${lib.escapeShellArg configDir}/config.toml

        mkdir -p "$config_dir"
        ln -s "$generated" "$config_dir/config.toml"

        # starting manga-tui loads (and would update) the config before handling the flag
        reported_dir=$(manga-tui --config-dir | tail -n 1)
        if [ "$reported_dir" != "$config_dir" ]; then
          echo "manga-tui uses $reported_dir as config directory, the module writes to $config_dir"
          exit 1
        fi

        # unlike startup, this command fails when the config file can't be deserialized
        manga-tui anilist check

        if [ "$(readlink "$config_dir/config.toml")" != "$generated" ]; then
          echo "manga-tui replaced the config file managed by Home Manager"
          exit 1
        fi

        # negative control: an invalid value must make the previous command fail
        rm "$config_dir/config.toml"
        sed 's/^download_type = .*/download_type = "zip"/' "$generated" > "$config_dir/config.toml"
        chmod 444 "$config_dir/config.toml"
        if manga-tui anilist check; then
          echo "manga-tui accepted an invalid config, this check does not test anything"
          exit 1
        fi

        touch $out
      '';
}
