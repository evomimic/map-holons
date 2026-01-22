{
  description = "Flake for Holochain app development with Rust client and Node.js";

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

        # Tools needed at build time (e.g., C compilers, generators)
        nativeBuildInputs = [
          pkgs.pkg-config
          pkgs.cmake
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          pkgs.libclang
          pkgs.rustPlatform.bindgenHook
        ];

        # Libraries needed for linking native deps
        buildInputs = [
          pkgs.openssl
          pkgs.libsodium
        ] ++ pkgs.lib.optionals pkgs.stdenv.isLinux [
          pkgs.glib
          pkgs.gtk3         # ✅ Fix: provide gdk-3.0.pc for gdk-sys
          pkgs.gdk-pixbuf   # Optional, but common in GTK apps
        ] ++ pkgs.lib.optionals pkgs.stdenv.isDarwin [
          pkgs.bzip2
          pkgs.libiconv
          pkgs.llvmPackages.libunwind
        ];

        # Runtime and app-specific tools
        packages = with pkgs; [
          nodejs_22
          binaryen
        ];

        # Required by bindgen (used via gdk-sys)
        LIBCLANG_PATH = "${pkgs.libclang.lib}/lib";

        shellHook = ''
          export PS1='\[\033[1;34m\][holonix:\w]\$\[\033[0m\] '
        '';
      };
    };
  };
}