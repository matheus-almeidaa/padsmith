{
  description = "Padsmith — remapeamento de controles para Linux";
  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      eachSystem = nixpkgs.lib.genAttrs systems;
      forSystem = system: import nixpkgs { inherit system; };
    in
    {
      packages = eachSystem (
        system:
        let
          pkgs = forSystem system;
          daemon = pkgs.callPackage ./nix/daemon.nix { };
          frontend = pkgs.callPackage ./nix/frontend.nix { };
          ui = pkgs.callPackage ./nix/ui.nix { inherit frontend; };
        in
        {
          inherit daemon frontend ui;
          default = pkgs.symlinkJoin {
            name = "padsmith-0.1.0";
            paths = [
              daemon
              ui
            ];
            meta = {
              mainProgram = "padsmith-ui";
              platforms = pkgs.lib.platforms.linux;
            };
          };
        }
      );
      nixosModules.default = import ./nix/module.nix self;
      devShells = eachSystem (
        system:
        let
          pkgs = forSystem system;
          buildAppImage = pkgs.writeShellApplication {
            name = "build-appimage";
            runtimeInputs = [
              pkgs.podman
              pkgs.coreutils
            ];
            text = ''
              project_root="''${PADSMITH_PROJECT_ROOT:-$PWD}"
              if [[ ! -f "$project_root/scripts/build-appimage.sh" ]]; then
                printf 'Entre em nix develop na raiz do Padsmith para usar build-appimage.\n' >&2
                exit 1
              fi
              exec ${pkgs.bash}/bin/bash "$project_root/scripts/build-appimage.sh"
            '';
          };
          cleanProject = pkgs.writeShellApplication {
            name = "clean-project";
            runtimeInputs = [ pkgs.coreutils ];
            text = ''
              project_root="''${PADSMITH_PROJECT_ROOT:-$PWD}"
              if [[ ! -f "$project_root/scripts/clean-project.sh" ]]; then
                printf 'Entre em nix develop na raiz do Padsmith para usar clean-project.\n' >&2
                exit 1
              fi
              exec ${pkgs.bash}/bin/bash "$project_root/scripts/clean-project.sh" "$@"
            '';
          };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              buildAppImage
              cleanProject
              cargo
              rustc
              rustfmt
              clippy
              pkg-config
              systemd
              nodejs
              pnpm
              cargo-tauri
              nixfmt
              gtk3
              glib
              gsettings-desktop-schemas
              webkitgtk_4_1
              libsoup_3
              libayatana-appindicator
              librsvg
              openssl
              libnotify
            ];
            shellHook = ''
              # The tray loads AppIndicator with dlopen, outside normal link-time lookup.
              export LD_LIBRARY_PATH="${
                pkgs.lib.makeLibraryPath [ pkgs.libayatana-appindicator ]
              }''${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
              # WebKitGTK needs these schemas to determine the display scale correctly.
              if [ -n "''${GSETTINGS_SCHEMAS_PATH:-}" ]; then
                export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH''${XDG_DATA_DIRS:+:$XDG_DATA_DIRS}"
              fi
              if [ -f ui/static/padsmith.svg ] && [ -d crates/ui/icons ]; then
                rsvg-convert -w 128 -h 128 ui/static/padsmith.svg -o crates/ui/icons/icon.png
                export TAURI_APP_PATH="$PWD/crates/ui"
                export TAURI_FRONTEND_PATH="$PWD/ui"
                export PADSMITH_PROJECT_ROOT="$PWD"
              fi
            '';
          };
        }
      );
      formatter = eachSystem (system: (forSystem system).nixfmt);
      checks = eachSystem (
        system:
        let
          pkgs = forSystem system;
          evaluated = nixpkgs.lib.nixosSystem {
            inherit system;
            modules = [
              self.nixosModules.default
              {
                system.stateVersion = "26.05";
                services.padsmith = {
                  enable = true;
                  devices = [
                    {
                      vendor = "054c";
                      product = "09cc";
                    }
                    {
                      vendor = "057e";
                      product = "2009";
                    }
                  ];
                  hideHidraw = true;
                };
              }
            ];
          };
          rules = evaluated.config.services.padsmith.rulesPackage;
        in
        {
          daemon = self.packages.${system}.daemon;
          frontend = self.packages.${system}.frontend;
          package = self.packages.${system}.default;
          udev = pkgs.runCommand "padsmith-udev-check" { nativeBuildInputs = [ pkgs.systemd ]; } ''
            udevadm verify ${rules}/lib/udev/rules.d/*.rules
            test '${evaluated.config.systemd.user.services.padsmith.serviceConfig.Type}' = notify
            test '${evaluated.config.systemd.user.services.padsmith.serviceConfig.WatchdogSec}' = 5s
            touch $out
          '';
        }
      );
    };
}
