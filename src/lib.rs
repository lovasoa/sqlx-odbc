//! ODBC database driver building blocks for SQLx.
//!
//! This crate is being ported as an independent split driver crate. It intentionally depends on
//! published crates from crates.io only; it does not depend on a local SQLx checkout.
//!
//! ## Test Setup
//!
//! Fast tests do not require an ODBC data source:
//!
//! ```sh
//! cargo test
//! ```
//!
//! Integration smoke tests use `ODBC_DATABASE_URL` and skip cleanly when it is absent:
//!
//! ```sh
//! ODBC_DATABASE_URL='DSN=MyDataSource;UID=user;PWD=password' cargo test --test odbc
//! ```
//!
//! To run the same tests locally against a known installed driver:
//!
//! ```sh
//! DUCKDB_ODBC_DRIVER=/absolute/path/to/libduckdb_odbc.so scripts/test-driver.sh duckdb
//! scripts/test-driver.sh sqlite
//! ```
//!
//! `ODBC_DATABASE_URL` may be a standard ODBC connection string, a bare DSN name, or the legacy
//! `odbc:` form:
//!
//! ```text
//! DSN=MyDataSource
//! Driver={ODBC Driver 17 for SQL Server};Server=localhost;Database=test
//! FILEDSN=/path/to/file.dsn
//! odbc:DSN=MyDataSource
//! MyDataSource
//! ```
//!
//! Native requirements are provided by the operating system:
//!
//! - Unix-like systems need an ODBC driver manager such as `unixODBC` or `iODBC`.
//! - A database-specific ODBC driver must be installed and visible to the driver manager.
//! - DSN names must be configured in the driver manager's usual files or registry locations.
//! - Buffered fetching can truncate long text or binary values when `max_column_size` is set.

#![deny(missing_docs)]
#![deny(rustdoc::broken_intra_doc_links)]
#![warn(future_incompatible, rust_2018_idioms)]

pub mod any;
mod arguments;
mod column;
mod connection;
mod database;
mod error;
mod options;
mod query_result;
mod row;
mod statement;
mod transaction;
mod type_info;
mod value;

pub use arguments::{OdbcArgumentValue, OdbcArguments, OdbcParameterCollection};
pub use column::OdbcColumn;
pub use connection::OdbcConnection;
pub use database::Odbc;
pub use error::{OdbcDatabaseError, OdbcError, Result};
pub use options::{OdbcBufferSettings, OdbcConnectOptions};
pub use query_result::OdbcQueryResult;
pub use row::OdbcRow;
pub use statement::OdbcStatement;
pub use transaction::OdbcTransactionManager;
pub use type_info::{DataTypeExt, OdbcTypeInfo};
pub use value::{OdbcValue, OdbcValueKind};

/// An alias for [`Pool`][sqlx_core::pool::Pool], specialized for ODBC.
pub type OdbcPool = sqlx_core::pool::Pool<Odbc>;

/// An alias for [`PoolOptions`][sqlx_core::pool::PoolOptions], specialized for ODBC.
pub type OdbcPoolOptions = sqlx_core::pool::PoolOptions<Odbc>;

/// An alias for [`Transaction`][sqlx_core::transaction::Transaction], specialized for ODBC.
pub type OdbcTransaction<'c> = sqlx_core::transaction::Transaction<'c, Odbc>;

/// An alias for [`Executor<'_, Database = Odbc>`][sqlx_core::executor::Executor].
pub trait OdbcExecutor<'c>: sqlx_core::executor::Executor<'c, Database = Odbc> {}
impl<'c, T> OdbcExecutor<'c> for T where T: sqlx_core::executor::Executor<'c, Database = Odbc> {}
