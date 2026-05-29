# sqlx-odbc Test Setup

This crate is intentionally independent from the local SQLx repository. It must depend only on
crates published to crates.io and must not use `path =` dependencies.

Run fast unit tests:

```sh
cargo test
```

Run the ODBC integration tests:

```sh
ODBC_DATABASE_URL='DSN=MyDataSource;UID=user;PWD=password' cargo test --test odbc
```

If `ODBC_DATABASE_URL` is unset or blank, the integration test prints a skip message and exits
successfully. The value may be a standard ODBC connection string, a bare DSN name, or `odbc:`
prefixed for legacy compatibility.

Driver scripts set `ODBC_TEST_REQUIRED=1`, so they fail instead of skipping if
they cannot produce a connection URL.

Run the same integration tests locally against one known driver:

```sh
scripts/test-driver.sh duckdb
DUCKDB_ODBC_DRIVER=/absolute/path/to/libduckdb_odbc.so scripts/test-driver.sh duckdb
scripts/test-driver.sh postgres
ODBC_DATABASE_URL='DSN=MyDataSource;UID=user;PWD=password' scripts/test-driver.sh custom
```

The DuckDB script target defaults to the registered `DuckDB` driver name. Set
`DUCKDB_ODBC_DRIVER` when the driver is not registered or when you want to force a specific driver
library path. On macOS, that path normally points to `libduckdb_odbc.dylib` instead of the Linux
`.so` file.

The PostgreSQL target expects a reachable PostgreSQL server and the PostgreSQL
ODBC driver registered with unixODBC. It defaults to
`Driver={PostgreSQL Unicode};Server=localhost;Port=5432;Database=postgres;Uid=postgres;Pwd=postgres`.
Override the pieces with `POSTGRES_ODBC_DRIVER`, `POSTGRES_HOST`,
`POSTGRES_PORT`, `POSTGRES_DATABASE`, `POSTGRES_USER`, and
`POSTGRES_PASSWORD`.

The Rust tests intentionally read only `ODBC_DATABASE_URL`. CI covers multiple actual drivers with
a job matrix that invokes `scripts/test-driver.sh` once per driver.

Native requirements:

- Unix-like systems need the `unixODBC` driver manager unless the
  `vendored-unix-odbc` feature is enabled.
- A database-specific ODBC driver must be installed and discoverable by the driver manager.
- DSNs must be configured in the driver manager's usual locations.
- Buffered fetching can truncate long text or binary values when `max_column_size` is set.
