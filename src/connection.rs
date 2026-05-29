use crate::{
    OdbcArguments, OdbcColumn, OdbcConnectOptions, OdbcParameterCollection, OdbcQueryResult,
    OdbcRow, OdbcStatement, OdbcTypeInfo, OdbcValue, OdbcValueKind, Result,
};
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{future, stream, StreamExt};
use odbc_api::{Cursor, DataType, Nullable, ResultSetMetadata};
use sqlx_core::column::Column;
use sqlx_core::executor::{Execute, Executor};
use sqlx_core::transaction::Transaction;
use sqlx_core::Either;
use std::future::Future;

/// Blocking ODBC connection wrapper.
///
/// This is the minimal smoke-test surface. The SQLx async `Connection` and `Executor` traits will
/// be implemented as the port progresses.
pub struct OdbcConnection {
    conn: odbc_api::Connection<'static>,
    transaction_depth: usize,
}

impl std::fmt::Debug for OdbcConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OdbcConnection").finish_non_exhaustive()
    }
}

impl OdbcConnection {
    /// Opens a blocking ODBC connection with the provided options.
    pub fn connect_blocking(options: &OdbcConnectOptions) -> Result<Self> {
        let env = odbc_api::environment()
            .map_err(|error| crate::OdbcError::Configuration(error.to_string()))?;

        let conn =
            env.connect_with_connection_string(options.connection_string(), Default::default())?;

        Ok(Self {
            conn,
            transaction_depth: 0,
        })
    }

    /// Executes a minimal connectivity query.
    pub fn ping_blocking(&mut self) -> Result<()> {
        self.conn.execute("SELECT 1", (), None)?;
        Ok(())
    }

    /// Returns the DBMS name reported by the ODBC driver.
    pub fn dbms_name(&self) -> Result<String> {
        Ok(self.conn.database_management_system_name()?)
    }

    pub(crate) fn begin_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth > 0 {
            return Err(sqlx_core::Error::InvalidSavePointStatement);
        }

        self.conn
            .set_autocommit(false)
            .map_err(crate::OdbcError::from)?;
        self.transaction_depth = 1;
        Ok(())
    }

    pub(crate) fn commit_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth == 0 {
            return Ok(());
        }

        self.conn.commit().map_err(crate::OdbcError::from)?;
        self.conn
            .set_autocommit(true)
            .map_err(crate::OdbcError::from)?;
        self.transaction_depth = 0;
        Ok(())
    }

    pub(crate) fn rollback_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth == 0 {
            return Ok(());
        }

        self.conn.rollback().map_err(crate::OdbcError::from)?;
        self.conn
            .set_autocommit(true)
            .map_err(crate::OdbcError::from)?;
        self.transaction_depth = 0;
        Ok(())
    }

    pub(crate) fn start_rollback(&mut self) {
        if self.transaction_depth == 0 {
            return;
        }

        if self.conn.rollback().is_ok() {
            let _ = self.conn.set_autocommit(true);
            self.transaction_depth = 0;
        }
    }

    pub(crate) const fn transaction_depth(&self) -> usize {
        self.transaction_depth
    }

    /// Prepares a statement and returns the metadata reported by the ODBC driver.
    pub fn prepare_blocking(
        &mut self,
        sql: sqlx_core::sql_str::SqlStr,
    ) -> std::result::Result<OdbcStatement, sqlx_core::Error> {
        let mut prepared = self
            .conn
            .prepare(sql.as_str())
            .map_err(crate::OdbcError::from)?;
        let parameters = prepared.num_params().map_err(crate::OdbcError::from)?;
        let columns = collect_prepared_columns(&mut prepared, parameters)?;

        Ok(OdbcStatement::new(sql, columns, usize::from(parameters)))
    }

    fn run_blocking_sql(
        &mut self,
        sql: &str,
        arguments: Option<&OdbcArguments>,
    ) -> std::result::Result<OdbcExecution, sqlx_core::Error> {
        let mut statement = self.conn.preallocate().map_err(crate::OdbcError::from)?;
        let parameters = odbc_parameters(arguments);

        if let Some(mut cursor) = statement
            .execute(sql, parameters.as_slice())
            .map_err(crate::OdbcError::from)?
        {
            return collect_rows(&mut cursor).map(OdbcExecution::Rows);
        }

        let rows_affected = statement
            .row_count()
            .map_err(crate::OdbcError::from)?
            .unwrap_or(0);

        let rows_affected = rows_affected.try_into().map_err(|_| {
            sqlx_core::Error::Protocol("ODBC row count does not fit in u64".to_owned())
        })?;

        Ok(OdbcExecution::Done(OdbcQueryResult::new(rows_affected)))
    }
}

impl sqlx_core::connection::Connection for OdbcConnection {
    type Database = crate::Odbc;
    type Options = OdbcConnectOptions;

    async fn close(self) -> std::result::Result<(), sqlx_core::Error> {
        drop(self);
        Ok(())
    }

    async fn close_hard(self) -> std::result::Result<(), sqlx_core::Error> {
        drop(self);
        Ok(())
    }

    async fn ping(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        self.ping_blocking().map_err(Into::into)
    }

    fn begin(
        &mut self,
    ) -> impl Future<Output = std::result::Result<Transaction<'_, Self::Database>, sqlx_core::Error>>
           + Send
           + '_ {
        Transaction::begin(self, None)
    }

    fn shrink_buffers(&mut self) {}

    async fn flush(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        Ok(())
    }

    fn should_flush(&self) -> bool {
        false
    }
}

impl<'c> Executor<'c> for &'c mut OdbcConnection {
    type Database = crate::Odbc;

    fn fetch_many<'e, 'q, E>(
        self,
        mut query: E,
    ) -> BoxStream<'e, std::result::Result<Either<OdbcQueryResult, OdbcRow>, sqlx_core::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        let arguments = query.take_arguments().map_err(sqlx_core::Error::Encode);
        let sql = query.sql();

        stream::once(async move {
            let arguments = arguments?;
            self.run_blocking_sql(sql.as_str(), arguments.as_ref())
        })
        .map(|result| match result {
            Ok(OdbcExecution::Done(result)) => {
                stream::once(future::ready(Ok(Either::Left(result)))).boxed()
            }
            Ok(OdbcExecution::Rows(rows)) => {
                stream::iter(rows.into_iter().map(|row| Ok(Either::Right(row)))).boxed()
            }
            Err(error) => stream::once(future::ready(Err(error))).boxed(),
        })
        .flatten()
        .boxed()
    }

    fn fetch_optional<'e, 'q, E>(
        self,
        mut query: E,
    ) -> BoxFuture<'e, std::result::Result<Option<OdbcRow>, sqlx_core::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
        'q: 'e,
        E: 'q,
    {
        let arguments = query.take_arguments().map_err(sqlx_core::Error::Encode);
        let sql = query.sql();

        Box::pin(async move {
            let arguments = arguments?;

            match self.run_blocking_sql(sql.as_str(), arguments.as_ref())? {
                OdbcExecution::Rows(rows) => Ok(rows.into_iter().next()),
                OdbcExecution::Done(_) => Ok(None),
            }
        })
    }

    fn prepare_with<'e>(
        self,
        sql: sqlx_core::sql_str::SqlStr,
        _parameters: &[crate::OdbcTypeInfo],
    ) -> BoxFuture<'e, std::result::Result<OdbcStatement, sqlx_core::Error>>
    where
        'c: 'e,
    {
        Box::pin(async move { self.prepare_blocking(sql) })
    }
}

enum OdbcExecution {
    Done(OdbcQueryResult),
    Rows(Vec<OdbcRow>),
}

fn odbc_parameters(arguments: Option<&OdbcArguments>) -> OdbcParameterCollection {
    arguments
        .map(OdbcArguments::to_odbc_parameter_collection)
        .unwrap_or_default()
}

fn collect_columns(
    cursor: &mut impl ResultSetMetadata,
) -> std::result::Result<Vec<OdbcColumn>, sqlx_core::Error> {
    let count = cursor.num_result_cols().map_err(crate::OdbcError::from)?;
    let count = usize::try_from(count).map_err(|_| {
        sqlx_core::Error::Protocol(format!("ODBC returned a negative column count: {count}"))
    })?;

    let mut columns = Vec::with_capacity(count);
    for ordinal in 0..count {
        let column_number = u16::try_from(ordinal + 1).map_err(|_| {
            sqlx_core::Error::Protocol(format!("ODBC column index exceeds u16: {}", ordinal + 1))
        })?;

        let mut description = odbc_api::ColumnDescription::default();
        cursor
            .describe_col(column_number, &mut description)
            .map_err(crate::OdbcError::from)?;
        let name = description
            .name_to_string()
            .unwrap_or_else(|_| format!("col{ordinal}"));

        columns.push(OdbcColumn::new(
            ordinal,
            name,
            OdbcTypeInfo::new(description.data_type),
        ));
    }

    Ok(columns)
}

fn collect_prepared_columns(
    prepared: &mut impl PreparedStatementMetadata,
    parameter_count: u16,
) -> std::result::Result<Vec<OdbcColumn>, sqlx_core::Error> {
    match collect_columns(prepared) {
        Ok(columns) => Ok(columns),
        Err(error) if parameter_count > 0 => {
            validate_parameter_metadata(prepared, parameter_count)?;
            log::debug!("ODBC driver deferred result-column metadata until execution: {error}");
            Ok(Vec::new())
        }
        Err(error) => Err(error),
    }
}

trait PreparedStatementMetadata: ResultSetMetadata {
    fn describe_prepared_parameter(
        &mut self,
        index: u16,
    ) -> std::result::Result<(), odbc_api::Error>;
}

impl<S> PreparedStatementMetadata for odbc_api::Prepared<S>
where
    S: odbc_api::handles::AsStatementRef,
{
    fn describe_prepared_parameter(
        &mut self,
        index: u16,
    ) -> std::result::Result<(), odbc_api::Error> {
        self.describe_param(index).map(|_| ())
    }
}

fn validate_parameter_metadata(
    prepared: &mut impl PreparedStatementMetadata,
    parameter_count: u16,
) -> std::result::Result<(), sqlx_core::Error> {
    for index in 1..=parameter_count {
        prepared
            .describe_prepared_parameter(index)
            .map_err(crate::OdbcError::from)?;
    }

    Ok(())
}

fn collect_rows(cursor: &mut impl Cursor) -> std::result::Result<Vec<OdbcRow>, sqlx_core::Error> {
    let columns = collect_columns(cursor)?;
    let mut rows = Vec::new();

    while let Some(mut cursor_row) = cursor.next_row().map_err(crate::OdbcError::from)? {
        let mut values = Vec::with_capacity(columns.len());

        for column in &columns {
            let column_number = u16::try_from(sqlx_core::column::Column::ordinal(column) + 1)
                .map_err(|_| {
                    sqlx_core::Error::Protocol("ODBC column index exceeds u16".to_owned())
                })?;
            values.push(fetch_value(
                &mut cursor_row,
                column_number,
                column.type_info().data_type(),
            )?);
        }

        rows.push(OdbcRow::new(columns.clone(), values));
    }

    Ok(rows)
}

fn fetch_value(
    row: &mut odbc_api::CursorRow<'_>,
    column_number: u16,
    data_type: DataType,
) -> std::result::Result<OdbcValue, sqlx_core::Error> {
    let kind = match data_type {
        DataType::Bit => {
            let mut value = Nullable::<odbc_api::Bit>::null();
            row.get_data(column_number, &mut value)
                .map_err(crate::OdbcError::from)?;
            value
                .into_opt()
                .map(|value| OdbcValueKind::Bit(value.as_bool()))
                .unwrap_or(OdbcValueKind::Null)
        }
        DataType::TinyInt => fetch_nullable(row, column_number, OdbcValueKind::TinyInt)?,
        DataType::SmallInt => fetch_nullable(row, column_number, OdbcValueKind::SmallInt)?,
        DataType::Integer => fetch_nullable(row, column_number, OdbcValueKind::Integer)?,
        DataType::BigInt => fetch_nullable(row, column_number, OdbcValueKind::BigInt)?,
        DataType::Real => fetch_nullable(row, column_number, OdbcValueKind::Real)?,
        DataType::Float { .. } | DataType::Double => {
            fetch_nullable(row, column_number, OdbcValueKind::Double)?
        }
        DataType::Date => fetch_nullable(row, column_number, OdbcValueKind::Date)?,
        DataType::Time { .. } => fetch_nullable(row, column_number, OdbcValueKind::Time)?,
        DataType::Timestamp { .. } => fetch_nullable(row, column_number, OdbcValueKind::Timestamp)?,
        DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. } => {
            let mut value = Vec::new();
            if row
                .get_binary(column_number, &mut value)
                .map_err(crate::OdbcError::from)?
            {
                OdbcValueKind::Binary(value)
            } else {
                OdbcValueKind::Null
            }
        }
        _ => {
            let mut value = Vec::new();
            if row
                .get_wide_text(column_number, &mut value)
                .map_err(crate::OdbcError::from)?
            {
                OdbcValueKind::Text(String::from_utf16_lossy(&value))
            } else {
                OdbcValueKind::Null
            }
        }
    };

    Ok(OdbcValue::new(kind))
}

fn fetch_nullable<T, F>(
    row: &mut odbc_api::CursorRow<'_>,
    column_number: u16,
    map: F,
) -> std::result::Result<OdbcValueKind, sqlx_core::Error>
where
    T: Default + Copy + odbc_api::parameter::CElement + odbc_api::handles::CDataMut,
    Nullable<T>: odbc_api::parameter::CElement + odbc_api::handles::CDataMut,
    F: FnOnce(T) -> OdbcValueKind,
{
    let mut value = Nullable::<T>::null();
    row.get_data(column_number, &mut value)
        .map_err(crate::OdbcError::from)?;
    Ok(value.into_opt().map(map).unwrap_or(OdbcValueKind::Null))
}
