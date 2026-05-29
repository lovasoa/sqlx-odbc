use crate::{Odbc, OdbcConnection};

/// Transaction manager placeholder for ODBC.
pub struct OdbcTransactionManager;

impl sqlx_core::transaction::TransactionManager for OdbcTransactionManager {
    type Database = Odbc;

    async fn begin(
        conn: &mut OdbcConnection,
        _statement: Option<sqlx_core::sql_str::SqlStr>,
    ) -> Result<(), sqlx_core::Error> {
        conn.begin_blocking()
    }

    async fn commit(conn: &mut OdbcConnection) -> Result<(), sqlx_core::Error> {
        conn.commit_blocking()
    }

    async fn rollback(conn: &mut OdbcConnection) -> Result<(), sqlx_core::Error> {
        conn.rollback_blocking()
    }

    fn start_rollback(conn: &mut OdbcConnection) {
        conn.start_rollback();
    }

    fn get_transaction_depth(conn: &OdbcConnection) -> usize {
        conn.transaction_depth()
    }
}
