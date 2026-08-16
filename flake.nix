{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-parts.url = "github:hercules-ci/flake-parts";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = inputs:
    let
      flakeModule = { ... }: {
        perSystem = { lib, pkgs, self', ... }:
          let
            nativeSource = lib.fileset.toSource {
              root = ./.;
              fileset = lib.fileset.unions [
                ./Cargo.lock
                ./Cargo.toml
                ./lib
                ./worker
              ];
            };

            nativeRuntimeDeps = with pkgs; [
              glib
            ];

            nativeBuildDeps = with pkgs; [
              pkg-config
              rustPlatform.bindgenHook
            ];

            nativeRustPackage = features:
              (pkgs.makeRustPlatform {
                cargo = pkgs.rust-bin.stable.latest.minimal;
                rustc = pkgs.rust-bin.stable.latest.minimal;
              }).buildRustPackage {
                pname = "quaver-scripting-native";
                version = "0.1.0";
                src = nativeSource;
                cargoLock.lockFile = ./Cargo.lock;
                buildFeatures = features;
                buildInputs = nativeRuntimeDeps;
                nativeBuildInputs = nativeBuildDeps;
              };

            mkNativeDevShell = rustc:
              pkgs.mkShell {
                shellHook = ''
                  export RUST_SRC_PATH=${pkgs.rustPlatform.rustLibSrc}
                  export LD_LIBRARY_PATH=${lib.makeLibraryPath nativeRuntimeDeps}:$LD_LIBRARY_PATH
                '';
                buildInputs = nativeRuntimeDeps;
                nativeBuildInputs = nativeBuildDeps ++ [ rustc ];
              };
          in {
            packages.quaver-scripting-native = nativeRustPackage [];

            devShells.native = self'.devShells.native-nightly;
            devShells.native-nightly = mkNativeDevShell (pkgs.rust-bin.selectLatestNightlyWith
              (toolchain: toolchain.default));
            devShells.native-stable = mkNativeDevShell pkgs.rust-bin.stable.latest.default;
          };
      };
    in
      inputs.flake-parts.lib.mkFlake { inherit inputs; } {
        imports = [ flakeModule ];

        systems = [ "x86_64-linux" ];

        flake.flakeModule = flakeModule;

        perSystem = { system, self', ... }: {
          _module.args.pkgs = import inputs.nixpkgs {
            inherit system;
            overlays = [
              (import inputs.rust-overlay)
            ];
          };

          packages.default = self'.packages.quaver-scripting-native;
          devShells.default = self'.devShells.native-nightly;
        };
      };
}
