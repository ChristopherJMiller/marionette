{
  description = "Marionette - Window manipulation MCP server for Linux";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Build dependencies for xcap and x11rb
        buildDeps = with pkgs; [
          pkg-config
          # X11 libraries
          xorg.libxcb
          xorg.libXrandr
          xorg.libX11
          xorg.libXext
          xorg.libXfixes
          # EGL/OpenGL/GBM for xcap
          libGL
          libglvnd
          libdrm
          libgbm
          # Wayland/portal dependencies for xcap
          dbus
          pipewire
          wayland
          # Clang for bindgen
          clang
          llvmPackages.libclang
        ];

        # Runtime dependencies
        runtimeDeps = with pkgs; [
          ydotool
        ];

        # Development tools
        devTools = with pkgs; [
          mcphost      # MCP CLI host for testing
          nodejs_22    # For npx @anthropic-ai/mcp-inspector
        ];
      in
      {
        devShells.default = pkgs.mkShell {
          buildInputs = buildDeps ++ runtimeDeps ++ devTools ++ [ rustToolchain ];

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
        };

        packages.default = pkgs.rustPlatform.buildRustPackage {
          pname = "marionette";
          version = "0.1.0";
          src = ./.;
          cargoLock.lockFile = ./Cargo.lock;

          nativeBuildInputs = with pkgs; [
            pkg-config
            clang
            makeWrapper
          ];
          buildInputs = buildDeps;

          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";

          # Wrap binary to include ydotool in PATH
          postInstall = ''
            wrapProgram $out/bin/marionette \
              --prefix PATH : ${pkgs.lib.makeBinPath runtimeDeps}
          '';

          meta = with pkgs.lib; {
            description = "Window manipulation MCP server for Linux";
            license = licenses.mit;
            platforms = platforms.linux;
          };
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/marionette";
        };
      }
    );
}
