use crate::{Odbc, OdbcArguments, OdbcColumn, OdbcTypeInfo};
use sqlx_core::sql_str::SqlStr;

/// Prepared statement metadata for ODBC.
#[derive(Debug, Clone)]
pub struct OdbcStatement {
    sql: SqlStr,
    columns: Vec<OdbcColumn>,
    parameters: usize,
}

impl OdbcStatement {
    /// Creates a statement metadata value.
    pub fn new(sql: SqlStr, columns: Vec<OdbcColumn>, parameters: usize) -> Self {
        Self {
            sql,
            columns,
            parameters,
        }
    }
}

impl sqlx_core::statement::Statement for OdbcStatement {
    type Database = Odbc;

    fn into_sql(self) -> SqlStr {
        self.sql
    }

    fn sql(&self) -> &SqlStr {
        &self.sql
    }

    fn parameters(&self) -> Option<sqlx_core::Either<&[OdbcTypeInfo], usize>> {
        Some(sqlx_core::Either::Right(self.parameters))
    }

    fn columns(&self) -> &[OdbcColumn] {
        &self.columns
    }

    sqlx_core::impl_statement_query!(OdbcArguments);
}

impl sqlx_core::column::ColumnIndex<OdbcStatement> for usize {
    fn index(&self, statement: &OdbcStatement) -> Result<usize, sqlx_core::Error> {
        if *self >= statement.columns.len() {
            return Err(sqlx_core::Error::ColumnIndexOutOfBounds {
                index: *self,
                len: statement.columns.len(),
            });
        }

        Ok(*self)
    }
}

impl sqlx_core::column::ColumnIndex<OdbcStatement> for &str {
    fn index(&self, statement: &OdbcStatement) -> Result<usize, sqlx_core::Error> {
        if let Some(index) = statement
            .columns
            .iter()
            .position(|column| sqlx_core::column::Column::name(column) == *self)
        {
            return Ok(index);
        }

        statement
            .columns
            .iter()
            .position(|column| sqlx_core::column::Column::name(column).eq_ignore_ascii_case(self))
            .ok_or_else(|| sqlx_core::Error::ColumnNotFound((*self).to_owned()))
    }
}
