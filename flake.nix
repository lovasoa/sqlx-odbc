{
  description = "ODBC driver for SQLx";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs =
    { self, nixpkgs }:
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
