{
  description = "Build a cargo workspace";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (localSystem:
      let
        crossSystem = "x86_64-unknown-linux-gnu";

        pkgs = import nixpkgs { inherit crossSystem localSystem; };

        inherit (pkgs) lib;

        craneLib = crane.mkLib pkgs;
        src = craneLib.cleanCargoSource ./.;

        # Common arguments can be set here to avoid repeating them later
        commonArgs = rec {
          inherit src;
          strictDeps = true;

          buildInputs = with pkgs;
            [
              # Add additional build inputs here
              openssl
              wayland
              wayland-protocols
              wayland.dev
              xorg.libX11
              xorg.libXcursor
              xorg.libxcb
              libxkbcommon
              vulkan-loader
              vulkan-headers
              libGL
            ] ++ lib.optionals stdenv.isDarwin [
              # Additional darwin specific inputs can be set here
              libiconv
            ];

          nativeBuildInputs = with pkgs; [
            pkg-config
            wayland-scanner
            makeWrapper
          ];

          # Additional environment variables can be set directly
          # MY_CUSTOM_VAR = "some value";

          LD_LIBRARY_PATH =
            "$LD_LIBRARY_PATH:${lib.makeLibraryPath buildInputs}";
        };

        libPath = lib.makeLibraryPath
          (with pkgs; [ xdg-desktop-portal ] ++ commonArgs.buildInputs);

        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${localSystem}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        # Build *just* the cargo dependencies (of the entire workspace),
        # so we can reuse all of that work (e.g. via cachix) when running in CI
        # It is *highly* recommended to use something like cargo-hakari to avoid
        # cache misses when building individual top-level-crates
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        individualCrateArgs = commonArgs // {
          inherit cargoArtifacts;
          inherit (craneLib.crateNameFromCargoToml { inherit src; }) version;
          # NB: we disable tests since we'll run them all via cargo-nextest
          doCheck = false;
        };

        fileSetForCrate = crate:
          lib.fileset.toSource {
            root = ./.;
            fileset = lib.fileset.unions [
              ./Cargo.toml
              ./Cargo.lock
              ./workspace-hack
              ./graph-rs
              crate
            ];
          };

        # Build the top-level crates of the workspace as individual derivations.
        # This allows consumers to only depend on (and build) only what they need.
        # Though it is possible to build the entire workspace as a single derivation,
        # so this is left up to you on how to organize things
        burp = craneLib.buildPackage (individualCrateArgs // {
          pname = "burp";
          cargoExtraArgs = "-p burp";
          src = fileSetForCrate (lib.fileset.unions [ ./burp ./burp-gui ]);
        });
        burp-gui = craneLib.buildPackage (individualCrateArgs // {
          pname = "burp-gui";
          cargoExtraArgs = "-p burp-gui";
          src = fileSetForCrate (lib.fileset.unions [ ./burp ./burp-gui ]);
          postInstall = ''
            wrapProgram "$out/bin/burp-gui" --prefix LD_LIBRARY_PATH : "${libPath}"
          '';
        });
        graph-rs = craneLib.buildPackage (individualCrateArgs // {
          pname = "graph-rs";
          cargoExtraArgs = "-p graph-rs";
          src = fileSetForCrate ./graph-rs;
        });
      in {
        checks = {
          # Build the crates as part of `nix flake check` for convenience
          burp = burp;
          burp-gui = burp-gui;

          # Run clippy (and deny all warnings) on the workspace source,
          # again, reusing the dependency artifacts from above.
          #
          # Note that this is done as a separate derivation so that
          # we can block the CI if there are issues here, but not
          # prevent downstream consumers from building our crate by itself.
          my-workspace-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          my-workspace-doc =
            craneLib.cargoDoc (commonArgs // { inherit cargoArtifacts; });

          # Check formatting
          my-workspace-fmt = craneLib.cargoFmt { inherit src; };

          # Audit dependencies
          my-workspace-audit = craneLib.cargoAudit { inherit src advisory-db; };

          # Audit licenses
          my-workspace-deny = craneLib.cargoDeny { inherit src; };

          # Run tests with cargo-nextest
          # Consider setting `doCheck = false` on other crate derivations
          # if you do not want the tests to run twice
          my-workspace-nextest = craneLib.cargoNextest (commonArgs // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
          });

          # Ensure that cargo-hakari is up to date
          my-workspace-hakari = craneLib.mkCargoDerivation {
            inherit src;
            pname = "workspace-hakari";
            cargoArtifacts = null;
            doInstallCargoArtifacts = false;

            buildPhaseCargoCommand = ''
              cargo hakari generate --diff  # workspace-hack Cargo.toml is up-to-date
              cargo hakari manage-deps --dry-run  # all workspace crates depend on workspace-hack
              cargo hakari verify
            '';

            nativeBuildInputs = [ pkgs.cargo-hakari ];
          };
        };

        packages = {
          burp = burp;
          burp-gui = burp-gui;
          graph-rs = graph-rs;
          default = burp-gui;
        } // lib.optionalAttrs (!pkgs.stdenv.isDarwin) {
          my-workspace-llvm-coverage = craneLibLLvmTools.cargoLlvmCov
            (commonArgs // { inherit cargoArtifacts; });
        };

        apps = {
          burp = flake-utils.lib.mkApp { drv = burp; };
          burp-gui = flake-utils.lib.mkApp { drv = burp-gui; };
        };

        devShells.default = craneLib.devShell {
          # Inherit inputs from checks.
          checks = self.checks.${localSystem};

          # Additional dev-shell environment variables can be set directly
          # MY_CUSTOM_DEVELOPMENT_VAR = "something else";
          LD_LIBRARY_PATH = "${lib.makeLibraryPath commonArgs.buildInputs}";

          # Extra inputs can be added here; cargo and rustc are provided by default.
          packages = [
            pkgs.cargo-hakari
            pkgs.gdb
            pkgs.vscode-extensions.vadimcn.vscode-lldb.adapter
            pkgs.typst
            pkgs.tinymist
            pkgs.gnuplot
          ];
        };

      });
}
