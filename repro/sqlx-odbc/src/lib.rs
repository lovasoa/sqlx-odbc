//! sqlx-odbc from this checkout, under the names a reproduction script uses.
//!
//! `repro/sqlx-oldapi` exports the same names for sqlx-oldapi 0.6.59, so one script builds
//! against either.

pub use sqlx_core::any::AnyConnection;
pub use sqlx_core::column::Column;
pub use sqlx_core::connection::Connection;
pub use sqlx_core::error::Error;
pub use sqlx_core::executor::Executor;
pub use sqlx_core::query::query;
pub use sqlx_core::row::Row;
pub use sqlx_core::type_info::TypeInfo;
pub use sqlx_odbc::OdbcConnection;

use sqlx_core::sql_str::SqlStr;
use std::sync::Once;

/// SQL text in the form `Executor::prepare` takes.
pub type Sql = SqlStr;

/// Turns SQL text into the form `Executor::prepare` takes.
pub fn sql(text: &'static str) -> Sql {
    SqlStr::from_static(text)
}

/// Opens an `AnyConnection` from an ODBC connection string.
pub async fn connect_any(connection_string: &str) -> Result<AnyConnection, Error> {
    static INSTALL_ODBC_DRIVER: Once = Once::new();
    INSTALL_ODBC_DRIVER.call_once(|| {
        sqlx_core::any::driver::install_drivers(&[sqlx_odbc::any::DRIVER])
            .expect("the ODBC driver is installed only once");
    });

    AnyConnection::connect(&format!("odbc:{connection_string}")).await
}
