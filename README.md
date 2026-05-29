# sqlx-odbc

Independent ODBC driver crate for SQLx.

This crate is a standalone top-level crate. It depends only on crates published
to crates.io and is not wired into the `sqlx` facade crate, so examples use
`sqlx-core` directly.

## Minimal Query

```toml
[dependencies]
sqlx-odbc = "0.1"
sqlx-core = "=0.9.0"
tokio = { version = "1", features = ["macros", "rt"] }
```

```rust
use sqlx_core::connection::Connection;
use sqlx_core::row::Row;
use sqlx_odbc::OdbcConnection;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = OdbcConnection::connect("Driver=SQLite3;Database=/tmp/example.db").await?;

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

## ODBC Setup

ODBC has two native pieces outside this crate:

1. Install a driver manager. On Linux this is usually `unixODBC`; on macOS,
   `unixODBC` or `iODBC`; on Windows, the driver manager is built in.
2. Install a database-specific ODBC driver, then either configure a DSN or pass
   a full connection string using `Driver=...`.

Useful setup guides:

- unixODBC: <https://www.unixodbc.org/>
- iODBC: <https://www.iodbc.org/>
- DuckDB ODBC: <https://duckdb.org/docs/stable/clients/odbc/overview.html>
- SQLite ODBC: <http://www.ch-werner.de/sqliteodbc/>
- Microsoft ODBC Driver for SQL Server: <https://learn.microsoft.com/sql/connect/odbc/>

For local development, run fast tests with `./scripts/ci.sh`. Run e2e tests
against one installed driver with `./scripts/test-driver.sh duckdb`,
`./scripts/test-driver.sh sqlite`, or `ODBC_DATABASE_URL=... ./scripts/test-driver.sh custom`.

