{
  description = "Low-latency mechanical keyboard & mouse acoustic feedback daemon for Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};

        pythonEnv = pkgs.python3.withPackages (ps: with ps; [
          pygobject3
          pyyaml
        ]);

        clickclick = pkgs.rustPlatform.buildRustPackage {
          pname = "clickclick";
          version = "1.0.0";
          src = ./.;

          cargoLock = {
            lockFile = ./Cargo.lock;
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            wrapGAppsHook4
            gobject-introspection
          ];

          buildInputs = with pkgs; [
            alsa-lib
            gtk4
            libadwaita
            pythonEnv
          ];

          postInstall = ''
            mkdir -p $out/share/clickclick/presets
            cp -r presets/* $out/share/clickclick/presets/

            mkdir -p $out/share/applications
            cp ui/com.github.saschlik.ClickClick.desktop $out/share/applications/

            mkdir -p $out/lib/clickclick
            cp ui/clickclick_gui.py $out/lib/clickclick/

            mkdir -p $out/bin
            install -Dm755 bin/clickclick $out/bin/clickclick
            ln -sf clickclick $out/bin/clickclick-cli
            ln -sf clickclick $out/bin/clickclick-ctl
            install -Dm755 ui/clickclick_gui.py $out/bin/clickclick-settings
          '';

          meta = with pkgs.lib; {
            description = "Low-latency mechanical keyboard acoustic feedback daemon and Libadwaita GUI";
            homepage = "https://github.com/saschlik/clickclick";
            license = licenses.mit;
            platforms = platforms.linux;
            mainProgram = "clickclick";
          };
        };
      in
      {
        packages.default = clickclick;
        packages.clickclick = clickclick;

        apps.default = flake-utils.lib.mkApp {
          drv = clickclick;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            cargo
            rustc
            pkg-config
            wrapGAppsHook4
            gobject-introspection
          ];
          buildInputs = with pkgs; [
            alsa-lib
            gtk4
            libadwaita
            pythonEnv
          ];
        };
      }
    );
}
