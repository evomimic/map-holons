{
  description = "Flake for Holochain app development";

  inputs = {
    holonix.url = "github:holochain/holonix?ref=main-0.5";

    nixpkgs.follows = "holonix/nixpkgs";
    flake-parts.follows = "holonix/flake-parts";
  };

  outputs = inputs@{ flake-parts, ... }:
    flake-parts.lib.mkFlake { inherit inputs; } {
      systems = builtins.attrNames inputs.holonix.devShells;

      perSystem = { inputs', pkgs, ... }: {
        formatter = pkgs.nixpkgs-fmt;

        devShells = {
          default = pkgs.mkShell {
            # Pull in holonix dev shell
            inputsFrom = [ inputs'.holonix.devShells.default ];

            # Extra native tools (incl. libclang + libstdc++ for CI)
            nativeBuildInputs = [
              pkgs.libsodium
              pkgs.pkg-config
              pkgs.llvmPackages.libunwind
              pkgs.llvmPackages.libclang        # ✅ Required by bindgen
              pkgs.llvmPackages.clang-unwrapped # ✅ Needed to satisfy some crates
              pkgs.stdenv.cc.cc.lib             # ✅ Pulls in libstdc++.so
              pkgs.cmake
            ];

            packages = with pkgs; [
              nodejs_22
              binaryen
            ];

            shellHook = ''
              export PS1='\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '

              # Use nix-provided libclang + LLVM runtime
              export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
              export LD_LIBRARY_PATH="${pkgs.llvmPackages.llvm}/lib:$LD_LIBRARY_PATH"

              export CMAKE_ARGS="''${CMAKE_ARGS:-} -DCMAKE_POLICY_VERSION_MINIMUM=3.10"
              export CMAKE_BUILD_PARALLEL_LEVEL="''${CMAKE_BUILD_PARALLEL_LEVEL:-1}"
            '' + pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
              # macOS-specific build settings go here
            '';
          };
        };
      };
    };
}