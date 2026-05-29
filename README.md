# sqlx-odbc

Independent ODBC driver crate for SQLx.

This crate is a standalone top-level crate. It depends only on crates published
to crates.io and is not wired into the `sqlx` facade crate, so examples use
`sqlx-core` directly.

## Minimal Query

```toml
[dependencies]
sqlx-odbc = "0.0.1-alpha"
sqlx-core = "=0.9.0"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

```rust
use sqlx_core::connection::Connection;
use sqlx_core::row::Row;
use sqlx_odbc::OdbcConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = OdbcConnection::connect("Driver=DuckDB;Database=/tmp/example.duckdb").await?;

    let row = sqlx_core::query::query("SELECT 1")
        .fetch_one(&mut conn)
        .await?;

    let value: i32 = row.try_get(0)?;
    println!("{value}");

    conn.close().await?;
    Ok(())
}
```

`OdbcConnection::connect()` accepts a standard ODBC connection string, a bare
DSN name, or the legacy `odbc:` prefix.

## Any Driver

Install this crate explicitly before opening ODBC URLs through SQLx
`AnyConnection`:

```rust
use sqlx_core::connection::Connection;

sqlx_core::any::driver::install_drivers(&[sqlx_odbc::any::DRIVER])?;

let mut conn = sqlx_core::any::AnyConnection::connect(
    "odbc:Driver=DuckDB;Database=/tmp/example.duckdb",
)
.await?;
```

## ODBC Setup

ODBC uses two native pieces outside this crate:

1. A driver manager. On Linux and macOS this is usually `unixODBC`; on Windows,
   the driver manager is built in.
2. A database-specific ODBC driver. Install the driver, then either configure a
   DSN or pass a full connection string using `Driver=...`.

For the fastest local smoke test, install the DuckDB ODBC driver and run:

```sh
./scripts/test-driver.sh duckdb
```

To test the same suite through the PostgreSQL ODBC driver, start PostgreSQL
locally and run:

```sh
./scripts/test-driver.sh postgres
```

To test another installed driver, set a connection string and use `custom`:

```sh
ODBC_DATABASE_URL='Driver=ODBC Driver 18 for SQL Server;Server=localhost;TrustServerCertificate=yes' \
    ./scripts/test-driver.sh custom
```

DuckDB currently works through the default unbuffered fetch path. Its ODBC
driver rejects the row-array statement attributes used by buffered fetching, so
leave `OdbcConnectOptions::max_column_size(None)` for DuckDB.

Useful setup guides:

- unixODBC documentation index: <https://www.unixodbc.org/doc/>
- unixODBC command-line configuration guide: <https://www.unixodbc.org/odbcinst.html>
- odbc-api installation notes: <https://docs.rs/odbc-api/latest/odbc_api/#installation>
- DuckDB ODBC: <https://duckdb.org/docs/stable/clients/odbc/overview.html>
- Microsoft ODBC Driver for SQL Server: <https://learn.microsoft.com/sql/connect/odbc/download-odbc-driver-for-sql-server>

## Static unixODBC

If you ship an application on Linux or macOS and do not want users to install
the `unixODBC` driver manager separately, enable this crate's vendored feature:

```toml
[dependencies]
sqlx-odbc = { version = "0.0.1-alpha", features = ["vendored-unix-odbc"] }
sqlx-core = "=0.9.0"
```

That feature forwards to `odbc-api/vendored-unix-odbc`, which builds and
statically links the unixODBC driver manager through `odbc-sys`. It removes the
driver-manager install step on Linux and macOS, but your users still need the
database-specific ODBC driver itself and any configuration files or connection
string values that driver requires. Vendored unixODBC is LGPL-2.1-or-later, so
review the licensing implications before distributing statically linked
artifacts.

For local development, run fast checks with `./scripts/ci.sh`. Run e2e tests
against one installed driver with `./scripts/test-driver.sh duckdb` or
`ODBC_DATABASE_URL=... ./scripts/test-driver.sh custom`.
