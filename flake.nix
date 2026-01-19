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
                          pkgs.glib              
                          pkgs.gtk3 
                        ];

            packages = with pkgs; [
              nodejs_22
              binaryen
            ];

            shellHook =
              ''
                export PS1='\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '

                # Cross-platform: modern CMake policy + reduce configure flakiness
                export CMAKE_ARGS="''${CMAKE_ARGS:-} -DCMAKE_POLICY_VERSION_MINIMUM=3.10"
                export CMAKE_BUILD_PARALLEL_LEVEL="''${CMAKE_BUILD_PARALLEL_LEVEL:-1}"
              ''
              # macOS-only: use Apple's toolchain for native deps
              + pkgs.lib.optionalString pkgs.stdenv.isDarwin ''
                # Requires Xcode CLT: xcode-select --install
                export CC="$(xcrun -f clang)"
                export CXX="$(xcrun -f clang++)"
                export AR="$(xcrun -f ar)"
                export SDKROOT="$(xcrun --show-sdk-path)"
                : ''${MACOSX_DEPLOYMENT_TARGET:=12.0}

                export CMAKE_C_COMPILER="$CC"
                export CMAKE_CXX_COMPILER="$CXX"
                export CMAKE_GENERATOR="Unix Makefiles"
                export CMAKE_OSX_ARCHITECTURES="${if pkgs.stdenv.isAarch64 then "arm64" else "x86_64"}"
              '';
          };
        };
      };
    };
}