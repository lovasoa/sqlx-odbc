use crate::{
    OdbcArguments, OdbcColumn, OdbcConnection, OdbcQueryResult, OdbcRow, OdbcStatement,
    OdbcTransactionManager, OdbcTypeInfo, OdbcValue,
};

/// ODBC database marker for SQLx-core traits.
#[derive(Debug)]
pub struct Odbc;

impl sqlx_core::database::Database for Odbc {
    type Connection = OdbcConnection;
    type TransactionManager = OdbcTransactionManager;
    type Row = OdbcRow;
    type QueryResult = OdbcQueryResult;
    type Column = OdbcColumn;
    type TypeInfo = OdbcTypeInfo;
    type Value = OdbcValue;
    type ValueRef<'r> = crate::value::OdbcValueRef<'r>;
    type Arguments = OdbcArguments;
    type ArgumentBuffer = Vec<crate::OdbcArgumentValue>;
    type Statement = OdbcStatement;

    const NAME: &'static str = "ODBC";
    const URL_SCHEMES: &'static [&'static str] = &["odbc"];
}

impl sqlx_core::database::HasStatementCache for Odbc {}
