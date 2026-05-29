#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
crate_dir="$(cd -- "${script_dir}/.." && pwd)"
cd "$crate_dir"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/test-driver.sh <duckdb|sqlite|custom> [cargo-test-args...]

Runs the ODBC integration test against one configured driver by setting
ODBC_DATABASE_URL for this process.

Drivers:
  duckdb   Requires DUCKDB_ODBC_DRIVER=/absolute/path/to/libduckdb_odbc library
  sqlite   Uses SQLITE_ODBC_DRIVER, defaulting to SQLite3
  custom   Requires ODBC_DATABASE_URL to already be set
USAGE
}

if [[ $# -lt 1 ]]; then
    usage
    exit 2
fi

driver="$1"
shift

tmp_dir="$(mktemp -d)"
cleanup() {
    rm -rf "$tmp_dir"
}
trap cleanup EXIT

case "$driver" in
    duckdb)
        : "${DUCKDB_ODBC_DRIVER:?DUCKDB_ODBC_DRIVER must point to the DuckDB ODBC driver library}"
        export ODBC_DATABASE_URL="Driver=${DUCKDB_ODBC_DRIVER};Database=${tmp_dir}/sqlx-odbc.duckdb"
        ;;
    sqlite)
        sqlite_driver="${SQLITE_ODBC_DRIVER:-SQLite3}"
        export ODBC_DATABASE_URL="Driver=${sqlite_driver};Database=${tmp_dir}/sqlx-odbc.sqlite"
        ;;
    custom)
        : "${ODBC_DATABASE_URL:?ODBC_DATABASE_URL must be set for the custom driver}"
        ;;
    *)
        usage
        exit 2
        ;;
esac

echo "running sqlx-odbc integration tests with driver: ${driver}"
cargo test --test odbc "$@"
