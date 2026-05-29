# sqlx-odbc Test Setup

This crate is intentionally independent from the local SQLx repository. It must depend only on
crates published to crates.io and must not use `path =` dependencies.

Run fast unit tests:

```sh
cargo test
```

Run the ODBC smoke integration test:

```sh
ODBC_DATABASE_URL='DSN=MyDataSource;UID=user;PWD=password' cargo test --test odbc
```

If `ODBC_DATABASE_URL` is unset or blank, the integration test prints a skip message and exits
successfully. The value may be a standard ODBC connection string, a bare DSN name, or `odbc:`
prefixed for legacy compatibility.

Native requirements:

- Unix-like systems need an ODBC driver manager such as `unixODBC` or `iODBC`.
- A database-specific ODBC driver must be installed and discoverable by the driver manager.
- DSNs must be configured in the driver manager's usual locations.
- Buffered fetching can truncate long text or binary values when `max_column_size` is set.
