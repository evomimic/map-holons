{
  description = "Flake for Holochain app development with rust client and nodejs";

  inputs = {
    holonix.url = "github:holochain/holonix?ref=main-0.5";

    nixpkgs.follows = "holonix/nixpkgs";
    flake-parts.follows = "holonix/flake-parts";
  };

  outputs = inputs@{ flake-parts, ... }: flake-parts.lib.mkFlake { inherit inputs; } {
    systems = builtins.attrNames inputs.holonix.devShells;
    perSystem = { inputs', pkgs, ... }: {
      formatter = pkgs.nixpkgs-fmt;

        devShells.default = pkgs.mkShell {
          inputsFrom = [ inputs'.holonix.devShells.default ];

          # 1. Tools needed at build time (running the build)
          nativeBuildInputs = [
            pkgs.pkg-config # Helper to find libraries (OpenSSL)
            pkgs.cmake      # Helper to build C dependencies
          ] ++ (pkgs.lib.optionals pkgs.stdenv.isLinux [
             # Linux-specific build tools
             pkgs.libclang
             pkgs.rustPlatform.bindgenHook
          ]);

          # 2. Libraries needed for linking (the actual code)
          buildInputs = [
            pkgs.openssl    # Shared: Pre-built OpenSSL (saves compilation time on Mac too)
            pkgs.libsodium  # Shared: Holochain crypto dependency
          ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
            pkgs.glib
            pkgs.gtk3         # Fix for unit:test: gdk-3.0.pc for gdk-sys
            pkgs.gdk-pixbuf   # Optional, safe to include
          ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
             # Mac-specific system frameworks
             pkgs.bzip2
             pkgs.libiconv
             pkgs.llvmPackages.libunwind # Essential for wasmer (Holochain) to work properly on macOS
          ];

          packages = with pkgs; [
            nodejs_22
            binaryen
          ];

          # Env vars
          LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

          shellHook = ''
             export PS1='\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '
          '';
        };
    };
  };
}