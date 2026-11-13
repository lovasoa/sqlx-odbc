//! sqlx-oldapi 0.6.59, under the names a reproduction script uses.
//!
//! `repro/sqlx-odbc` exports the same names for sqlx-odbc, so one script builds against either.

pub use sqlx_core_oldapi::any::AnyConnection;
pub use sqlx_core_oldapi::column::Column;
pub use sqlx_core_oldapi::connection::Connection;
pub use sqlx_core_oldapi::error::Error;
pub use sqlx_core_oldapi::executor::Executor;
pub use sqlx_core_oldapi::odbc::OdbcConnection;
pub use sqlx_core_oldapi::query::query;
pub use sqlx_core_oldapi::row::Row;
pub use sqlx_core_oldapi::type_info::TypeInfo;

/// SQL text in the form `Executor::prepare` takes.
pub type Sql = &'static str;

/// Turns SQL text into the form `Executor::prepare` takes.
pub fn sql(text: &'static str) -> Sql {
    text
}

/// Opens an `AnyConnection` from an ODBC connection string.
pub async fn connect_any(connection_string: &str) -> Result<AnyConnection, Error> {
    AnyConnection::connect(connection_string).await
}
