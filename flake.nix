{
  description = "ODBC driver for SQLx";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      rust-overlay,
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forEachSystem =
        function: nixpkgs.lib.genAttrs systems (system: function nixpkgs.legacyPackages.${system});
    in
    {
      packages = forEachSystem (pkgs: rec {
        duckdb-odbc = pkgs.callPackage ./nix/duckdb-odbc.nix { };

        postgresql-odbc = pkgs.unixodbcDrivers.psql;

        odbcinst = pkgs.callPackage ./nix/odbcinst.nix {
          drivers = {
            "DuckDB" = duckdb-odbc;
            "PostgreSQL Unicode" = postgresql-odbc;
          };
        };

        postgresql = pkgs.postgresql_17;

        with-postgres = pkgs.callPackage ./nix/with-postgres.nix { inherit postgresql; };

        cargo-nightly = (rust-overlay.lib.mkRustBin { } pkgs).nightly."2026-11-11".minimal;

        repro = pkgs.callPackage ./nix/repro.nix {
          inherit
            cargo-nightly
            duckdb-odbc
            odbcinst
            postgresql
            postgresql-odbc
            with-postgres
            ;
          revision = self.rev or self.dirtyRev or null;
        };
      });

      apps = forEachSystem (pkgs: {
        repro = {
          type = "app";
          program = nixpkgs.lib.getExe self.packages.${pkgs.stdenv.hostPlatform.system}.repro;
          meta.description = "Run reproduction scripts against sqlx-oldapi and sqlx-odbc";
        };
      });

      devShells = forEachSystem (
        pkgs:
        let
          packages = self.packages.${pkgs.stdenv.hostPlatform.system};
        in
        {
          default = pkgs.mkShell {
            packages = [
              pkgs.cargo
              pkgs.clippy
              pkgs.rust-analyzer
              pkgs.rustc
              pkgs.rustfmt
              pkgs.unixodbc
              packages.postgresql
              packages.with-postgres
            ];

            ODBCSYSINI = packages.odbcinst;
            RUST_SRC_PATH = pkgs.rustPlatform.rustLibSrc;
          };
        }
      );

      formatter = forEachSystem (pkgs: pkgs.nixfmt-tree);
    };
}
