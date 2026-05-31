//! ODBC driver for SQLx.
//!
//! `sqlx-odbc` connects SQLx to databases exposed through an ODBC driver
//! manager. Use it directly with `sqlx-core` native APIs, or install its
//! [`Any` driver][any::DRIVER] when an application wants to open ODBC connection
//! strings through `AnyConnection`.
//!
//! # Native connection
//!
//! ```no_run
//! use sqlx_core::connection::Connection;
//! use sqlx_core::row::Row;
//! use sqlx_odbc::OdbcConnection;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! let mut conn = OdbcConnection::connect("Driver=DuckDB;Database=/tmp/example.duckdb").await?;
//!
//! let row = sqlx_core::query::query("SELECT 1")
//!     .fetch_one(&mut conn)
//!     .await?;
//!
//! let value: i32 = row.try_get(0)?;
//! assert_eq!(value, 1);
//!
//! conn.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! `OdbcConnection::connect()` accepts a standard ODBC connection string, a bare
//! DSN name, or the legacy `odbc:` prefix.
//!
//! # `AnyConnection`
//!
//! Install this driver before connecting through SQLx `Any` APIs:
//!
//! ```no_run
//! use sqlx_core::connection::Connection;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! sqlx_core::any::driver::install_drivers(&[sqlx_odbc::any::DRIVER])?;
//!
//! let mut conn = sqlx_core::any::AnyConnection::connect(
//!     "odbc:Driver=DuckDB;Database=/tmp/example.duckdb",
//! )
//! .await?;
//!
//! conn.close().await?;
//! # Ok(())
//! # }
//! ```
//!
//! To combine split drivers, install all of them once at application startup:
//!
//! ```ignore
//! fn install() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//! sqlx_core::any::driver::install_drivers(&[
//!     sqlx_sqlserver::any::DRIVER,
//!     sqlx_odbc::any::DRIVER,
//! ])?;
//! Ok(())
//! }
//! ```
//!
//! # Native ODBC requirements
//!
//! On Linux and macOS, install a driver manager such as unixODBC plus a
//! database-specific ODBC driver. On Windows, the driver manager is built in,
//! but the database-specific driver is still required. DSNs can be configured in
//! the driver manager, or callers can pass a full `Driver=...;...` connection
//! string.
//!
//! Enable the `vendored-unix-odbc` feature to statically link the unixODBC
//! driver manager into your application on Linux or macOS. The actual database
//! ODBC driver still needs to be installed and discoverable at runtime.
//!
//! Buffered fetching can improve throughput, but long text or binary values may
//! be truncated when `max_column_size` is set. Use unbuffered mode for values
//! that may exceed that limit.

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
mod types;
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

sqlx_core::impl_acquire!(Odbc, OdbcConnection);
