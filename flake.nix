{
  description = "A devShell example";
  inputs = {
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
    fenix.url = "github:nix-community/fenix";
    fenix.inputs.nixpkgs.follows = "nixpkgs";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };
  outputs = {
    self,
    nixpkgs,
    crane,
    flake-utils,
    fenix,
    advisory-db,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (system: let
      pkgs = import nixpkgs {
        inherit system;
        overlays = [fenix.overlays.default];
      };
      inherit (pkgs) lib;

      rustToolchain = pkgs.fenix.combine (with pkgs.fenix; [
        stable.cargo
        stable.clippy
        stable.rustc
        latest.rustfmt
      ]);

      craneLib = (crane.mkLib pkgs).overrideToolchain (p: rustToolchain);
      craneDev = craneLib.overrideToolchain (p:
        p.fenix.combine (with p.fenix.stable; [
          rustToolchain
          rust-analyzer
          rust-src
        ]));

      root = ./.;
      src = lib.fileset.toSource {
        inherit root;
        fileset = lib.fileset.unions [
          (craneLib.fileset.commonCargoSources ./.)
          (lib.fileset.maybeMissing ./rustfmt.toml)
        ];
      };

      rustHostPlatform = pkgs.hostPlatform.rust.rustcTarget;

      # Common arguments can be set here to avoid repeating them later
      commonArgs = {
        inherit src;
        strictDeps = true;

        buildInputs = with pkgs; [] ++ lib.optionals stdenv.isDarwin [];

        nativeBuildInputs = with pkgs; [];
      };

      mkChecks = craneLib: let
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in {
        runesys = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
            # Additional environment variables or build phases/hooks can be set
            # here *without* rebuilding all dependency crates
            # MY_CUSTOM_VAR = "some value";
            #
            doCheck = false;
          }
        );

        # Run clippy
        runesys-clippy = craneLib.cargoClippy (commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

        # Check formatting
        runesys-fmt = craneLib.cargoFmt {
          inherit src;
        };

        # Audit dependencies
        runesys-audit = craneLib.cargoAudit {
          inherit src advisory-db;
        };

        # Audit licenses
        runesys-deny = craneLib.cargoDeny {
          inherit src;
        };

        # Run tests with cargo-nextest
        # Consider setting `doCheck = false` on other crate derivations
        # if you do not want the tests to run twice
        runesys-nextest = craneLib.cargoNextest (commonArgs
          // {
            inherit cargoArtifacts;
            partitions = 1;
            partitionType = "count";
            cargoNextestPartitionsExtraArgs = "--no-tests=pass";
          });
      };
    in {
      checks = mkChecks craneLib;

      devShells.default = craneDev.devShell {
        checks = mkChecks craneDev;

        CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER = "${pkgs.llvmPackages.clangUseLLVM}/bin/clang";
        CARGO_ENCODED_RUSTFLAGS = "-Clink-arg=-fuse-ld=${pkgs.mold}/bin/mold";

        packages = with pkgs; [
          # sqlx-cli
        ];
      };
    });
}
