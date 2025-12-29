{
  description = "Rust development shell and build";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
  };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};

      # 1. Define the libraries required for eframe (Wayland/X11/OpenGL)
      runtimeLibs = with pkgs; [
        libxkbcommon
        libGL
        wayland
        xorg.libX11
        xorg.libXcursor
        xorg.libXi
        xorg.libXrandr
        alsa-lib
        libpulseaudio
      ];

      # 2. Create the library path string for linking
      libPath = pkgs.lib.makeLibraryPath runtimeLibs;
    in
    {
      # --- The Build Config (nix build & nix run) ---
      packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
        pname = "dnd_wheel";
        version = "0.1.0";
        src = ./.; # Uses the current directory as source

        # IMPORTANT: You must have a Cargo.lock file for this to work
        cargoLock.lockFile = ./Cargo.lock;

        # Tools needed during build time
        nativeBuildInputs = [ pkgs.pkg-config pkgs.makeWrapper ];

        # Libraries needed during build time
        buildInputs = runtimeLibs;

        # Post-install: Wrap the binary so it knows where to find libraries at runtime
        postInstall = ''
          wrapProgram $out/bin/dnd_wheel \
            --prefix LD_LIBRARY_PATH : "${libPath}"
        '';
      };

      # --- The App Config (nix run) ---
      apps.${system}.default = {
        type = "app";
        program = "${self.packages.${system}.default}/bin/dnd_wheel";
      };

      # --- The Dev Shell Config (nix develop) ---
      devShells.${system}.default = pkgs.mkShell {
        nativeBuildInputs = with pkgs; [
          pkg-config
          cargo
          rustc
          rustfmt
          clippy
          rust-analyzer
        ];

        buildInputs = runtimeLibs;

        # Environment variables for the shell
        RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";

        # This makes 'cargo run' work inside the shell
        LD_LIBRARY_PATH = libPath;
      };
    };
}
