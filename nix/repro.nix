{
  lib,
  stdenv,
  unixodbc,
  writeShellApplication,
  cargo-nightly,
  duckdb-odbc,
  odbcinst,
  postgresql,
  postgresql-odbc,
  with-postgres,
  revision,
}:

let
  source = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../README.md
      ../src
      ../repro/sqlx-oldapi/Cargo.toml
      ../repro/sqlx-oldapi/src
      ../repro/sqlx-odbc/Cargo.toml
      ../repro/sqlx-odbc/src
    ];
  };

  commit = lib.removeSuffix "-dirty" revision;
  sqlxOdbcLabel =
    if revision == null then
      "sqlx-odbc"
    else
      "sqlx-odbc ${builtins.substring 0 12 commit}"
      + lib.optionalString (lib.hasSuffix "-dirty" revision) " with uncommitted changes";

  run-reproductions = writeShellApplication {
    name = "run-reproductions";
    runtimeInputs = [
      cargo-nightly
      stdenv.cc
    ];
    text = ''
      export RUSTFLAGS="-L native=${unixodbc}/lib -C link-arg=-Wl,-rpath,${unixodbc}/lib"
      build_cache="''${XDG_CACHE_HOME:-$HOME/.cache}/sqlx-odbc-repro"

      driver_labels=("sqlx-oldapi 0.6.59" "${sqlxOdbcLabel}")
      driver_crates=("${source}/repro/sqlx-oldapi" "${source}/repro/sqlx-odbc")
      driver_build_dirs=("$build_cache/sqlx-oldapi" "$build_cache/sqlx-odbc")

      database_labels=(
          "DuckDB ODBC ${duckdb-odbc.version}"
          "PostgreSQL ${postgresql.version}, psqlODBC ${postgresql-odbc.version}"
      )
      database_urls=(
          "Driver=DuckDB;Database=:memory:"
          "Driver={PostgreSQL Unicode};Server=$POSTGRES_HOST;Port=$POSTGRES_PORT;Database=$POSTGRES_DATABASE;Uid=$POSTGRES_USER;Pwd=$POSTGRES_PASSWORD"
      )

      run_with_driver() {
          local script="$1" driver="$2" database="$3"

          # Cargo scripts are still unstable: https://github.com/rust-lang/cargo/issues/12207
          DATABASE_URL="''${database_urls[$database]}" \
          CARGO_BUILD_BUILD_DIR="''${driver_build_dirs[$driver]}" \
              cargo -Zscript --quiet \
                  --config "patch.\"https://github.com/lovasoa/sqlx-odbc\".sqlx-repro.path=\"''${driver_crates[$driver]}\"" \
                  "$script" 2>&1
      }

      for script in "$@"; do
          echo "==> cat $script"
          cat "$script"

          for driver in "''${!driver_labels[@]}"; do
              for database in "''${!database_labels[@]}"; do
                  echo
                  echo "==> ''${driver_labels[$driver]} on ''${database_labels[$database]}"
                  run_with_driver "$script" "$driver" "$database" || echo "(exit status $?)"
              done
          done
          echo
      done
    '';
  };
in
writeShellApplication {
  name = "repro";
  runtimeInputs = [
    with-postgres
    run-reproductions
  ];
  text = ''
    if [[ $# -eq 0 ]]; then
        stdin_dir="$(mktemp -d)"
        trap 'rm -rf "$stdin_dir"' EXIT
        cd "$stdin_dir"
        cat > reproduction.rs
        set -- reproduction.rs
    fi

    export ODBCSYSINI=${odbcinst}
    with-postgres run-reproductions "$@"
  '';
}
