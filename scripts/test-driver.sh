#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
crate_dir="$(cd -- "${script_dir}/.." && pwd)"
cd "$crate_dir"

usage() {
    cat >&2 <<'USAGE'
usage: scripts/test-driver.sh <duckdb|postgres|custom> [cargo-test-args...]

Runs the ODBC integration test against one configured driver by setting
ODBC_DATABASE_URL for this process.

Drivers:
  duckdb    Uses DUCKDB_ODBC_DRIVER, defaulting to the registered DuckDB driver
  postgres  Uses POSTGRES_* env vars, defaulting to local PostgreSQL
  custom    Requires ODBC_DATABASE_URL to already be set
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
        duckdb_driver="${DUCKDB_ODBC_DRIVER:-DuckDB}"
        export ODBC_DATABASE_URL="Driver=${duckdb_driver};Database=${tmp_dir}/sqlx-odbc.duckdb"
        set -- "$@" -- \
            --skip sqlx_query_fetches_basic_row_in_buffered_mode_when_configured \
            --skip sqlx_query_decodes_decimal_integer_in_buffered_mode_when_configured
        ;;
    postgres)
        postgres_driver="${POSTGRES_ODBC_DRIVER:-PostgreSQL Unicode}"
        postgres_host="${POSTGRES_HOST:-localhost}"
        postgres_port="${POSTGRES_PORT:-5432}"
        postgres_database="${POSTGRES_DATABASE:-postgres}"
        postgres_user="${POSTGRES_USER:-postgres}"
        postgres_password="${POSTGRES_PASSWORD:-postgres}"
        export ODBC_DATABASE_URL="Driver={${postgres_driver}};Server=${postgres_host};Port=${postgres_port};Database=${postgres_database};Uid=${postgres_user};Pwd=${postgres_password}"
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
