use crate::{
    OdbcArguments, OdbcBufferSettings, OdbcColumn, OdbcConnectOptions, OdbcParameterCollection,
    OdbcQueryResult, OdbcRow, OdbcStatement, OdbcTypeInfo, OdbcValue, OdbcValueKind, Result,
};
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{future, stream, StreamExt};
use odbc_api::buffers::{AnyColumnBufferSlice, BufferDesc, ColumnarDynBuffer, NullableSlice};
use odbc_api::{Cursor, DataType, Nullable, ResultSetMetadata};
use sqlx_core::column::Column;
use sqlx_core::executor::{Execute, Executor};
use sqlx_core::transaction::Transaction;
use sqlx_core::Either;
use std::future::Future;
use std::sync::Arc;

/// Blocking ODBC connection wrapper.
///
/// This is the minimal smoke-test surface. The SQLx async `Connection` and `Executor` traits will
/// be implemented as the port progresses.
pub struct OdbcConnection {
    conn: odbc_api::Connection<'static>,
    buffer_settings: OdbcBufferSettings,
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
        let env = odbc_api::environment().map_err(|error| {
            crate::OdbcError::Configuration(format!(
                "failed to initialize the process-wide ODBC environment: {error}"
            ))
        })?;

        let conn = env
            .connect_with_connection_string(options.connection_string(), Default::default())
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    "failed to open ODBC connection using the supplied connection string",
                )
            })?;

        Ok(Self {
            conn,
            buffer_settings: options.buffer_settings,
            transaction_depth: 0,
        })
    }

    /// Executes a minimal connectivity query.
    pub fn ping_blocking(&mut self) -> Result<()> {
        let query = self
            .conn
            .database_management_system_name()
            .map(|name| ping_query_for_dbms_name(&name))
            .unwrap_or("SELECT 1");
        self.conn.execute(query, (), None).map_err(|error| {
            crate::error::database_error_with_context(
                error,
                format!("ODBC ping query failed: `{query}`"),
            )
        })?;
        Ok(())
    }

    /// Returns the DBMS name reported by the ODBC driver.
    pub fn dbms_name(&self) -> Result<String> {
        self.conn
            .database_management_system_name()
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    "failed to read the ODBC DBMS name from SQLGetInfo",
                )
            })
    }

    pub(crate) fn begin_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth > 0 {
            return Err(sqlx_core::Error::InvalidSavePointStatement);
        }

        self.conn.set_autocommit(false).map_err(|error| {
            crate::error::database_error_with_context(
                error,
                "failed to disable ODBC autocommit while beginning a transaction",
            )
        })?;
        self.transaction_depth = 1;
        Ok(())
    }

    pub(crate) fn commit_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth == 0 {
            return Ok(());
        }

        self.conn.commit().map_err(|error| {
            crate::error::database_error_with_context(
                error,
                "failed to commit the active ODBC transaction",
            )
        })?;
        self.conn.set_autocommit(true).map_err(|error| {
            crate::error::database_error_with_context(
                error,
                "failed to restore ODBC autocommit after commit",
            )
        })?;
        self.transaction_depth = 0;
        Ok(())
    }

    pub(crate) fn rollback_blocking(&mut self) -> std::result::Result<(), sqlx_core::Error> {
        if self.transaction_depth == 0 {
            return Ok(());
        }

        self.conn.rollback().map_err(|error| {
            crate::error::database_error_with_context(
                error,
                "failed to roll back the active ODBC transaction",
            )
        })?;
        self.conn.set_autocommit(true).map_err(|error| {
            crate::error::database_error_with_context(
                error,
                "failed to restore ODBC autocommit after rollback",
            )
        })?;
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
        let mut prepared = self.conn.prepare(sql.as_str()).map_err(|error| {
            crate::error::database_error_with_context(
                error,
                format!(
                    "failed to prepare ODBC statement: `{}`",
                    sql_preview(sql.as_str())
                ),
            )
        })?;
        let parameters = prepared.num_params().map_err(|error| {
            crate::error::database_error_with_context(
                error,
                format!(
                    "failed to read ODBC parameter metadata for prepared statement: `{}`",
                    sql_preview(sql.as_str())
                ),
            )
        })?;
        let columns = collect_prepared_columns(&mut prepared, parameters)?;

        Ok(OdbcStatement::new(sql, columns, usize::from(parameters)))
    }

    pub(crate) fn run_blocking_sql(
        &mut self,
        sql: &str,
        arguments: Option<&OdbcArguments>,
    ) -> std::result::Result<OdbcExecution, sqlx_core::Error> {
        let mut statement = self.conn.preallocate().map_err(|error| {
            crate::error::database_error_with_context(
                error,
                format!(
                    "failed to allocate an ODBC statement for query: `{}`",
                    sql_preview(sql)
                ),
            )
        })?;
        let parameters = odbc_parameters(arguments);

        if let Some(cursor) = statement
            .execute(sql, parameters.as_slice())
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    format!("failed to execute ODBC query: `{}`", sql_preview(sql)),
                )
            })?
        {
            return collect_rows(cursor, self.buffer_settings).map(OdbcExecution::Rows);
        }

        let rows_affected = statement
            .row_count()
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    format!(
                        "failed to read ODBC row count for query: `{}`",
                        sql_preview(sql)
                    ),
                )
            })?
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
            Ok(OdbcExecution::Rows(rows)) => stream::iter(
                rows.into_iter()
                    .map(|row| Ok(Either::Right(row)))
                    .chain(std::iter::once(Ok(Either::Left(OdbcQueryResult::new(0))))),
            )
            .boxed(),
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

pub(crate) enum OdbcExecution {
    Done(OdbcQueryResult),
    Rows(Vec<OdbcRow>),
}

fn odbc_parameters(arguments: Option<&OdbcArguments>) -> OdbcParameterCollection {
    arguments
        .map(OdbcArguments::to_odbc_parameter_collection)
        .unwrap_or_default()
}

fn ping_query_for_dbms_name(dbms_name: &str) -> &'static str {
    let dbms_name = dbms_name.to_ascii_uppercase();

    if dbms_name.contains("DB2")
        || dbms_name.contains("DB/2")
        || dbms_name.contains("ISERIES")
        || dbms_name.contains("AS/400")
        || dbms_name.contains("IBM I")
    {
        "SELECT 1 FROM SYSIBM.SYSDUMMY1"
    } else {
        "SELECT 1"
    }
}

fn collect_columns(
    cursor: &mut impl ResultSetMetadata,
) -> std::result::Result<Vec<OdbcColumn>, sqlx_core::Error> {
    let count = cursor.num_result_cols().map_err(|error| {
        crate::error::database_error_with_context(error, "failed to read ODBC result-column count")
    })?;
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
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    format!("failed to describe ODBC result column {column_number}"),
                )
            })?;
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
            .map_err(|error| {
                crate::error::database_error_with_context(
                    error,
                    format!("failed to describe ODBC parameter {index}"),
                )
            })?;
    }

    Ok(())
}

fn collect_rows<C>(
    cursor: C,
    settings: OdbcBufferSettings,
) -> std::result::Result<Vec<OdbcRow>, sqlx_core::Error>
where
    C: Cursor + ResultSetMetadata,
{
    if let Some(max_column_size) = settings.max_column_size {
        collect_rows_buffered(cursor, settings.batch_size, max_column_size)
    } else {
        collect_rows_unbuffered(cursor)
    }
}

#[derive(Debug)]
struct ColumnBinding {
    column: OdbcColumn,
    buffer_desc: BufferDesc,
}

fn collect_rows_buffered<C>(
    cursor: C,
    batch_size: usize,
    max_column_size: usize,
) -> std::result::Result<Vec<OdbcRow>, sqlx_core::Error>
where
    C: Cursor + ResultSetMetadata,
{
    let mut cursor = cursor;
    let bindings = build_buffer_bindings(&mut cursor, max_column_size)?;
    let buffer_descriptions = bindings
        .iter()
        .map(|binding| binding.buffer_desc)
        .collect::<Vec<_>>();
    let mut row_set_cursor = cursor
        .bind_buffer(ColumnarDynBuffer::from_descs(
            batch_size,
            buffer_descriptions,
        ))
        .map_err(|error| {
            crate::error::database_error_with_context(
                error,
                format!(
                    "ODBC buffered fetching could not be enabled with batch_size={batch_size}; \
                     this driver may reject the row-array or row-binding statement attributes \
                     used for column-wise buffered fetching, so use \
                     OdbcConnectOptions::max_column_size(None) to fetch rows unbuffered"
                ),
            )
        })?;
    let columns: Arc<[OdbcColumn]> = bindings
        .iter()
        .map(|binding| binding.column.clone())
        .collect::<Vec<_>>()
        .into();
    let mut rows = Vec::new();

    while let Some(batch) = row_set_cursor.fetch().map_err(|error| {
        crate::error::database_error_with_context(error, "ODBC buffered fetch failed")
    })? {
        let column_values = bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| {
                buffered_column_values(batch.column(index), binding).map_err(|error| {
                    sqlx_core::Error::Protocol(format!(
                        "ODBC buffered fetch could not convert column {} (`{}`) using buffer {:?}: {error}",
                        binding.column.ordinal() + 1,
                        binding.column.name(),
                        binding.buffer_desc
                    ))
                })
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let mut column_iters = column_values
            .into_iter()
            .map(Vec::into_iter)
            .collect::<Vec<_>>();

        for row_index in 0..batch.num_rows() {
            let values = column_iters
                .iter_mut()
                .map(|values| {
                    values.next().map(OdbcValue::new).ok_or_else(|| {
                        sqlx_core::Error::Protocol(format!(
                            "ODBC buffered fetch produced too few values for row {}",
                            row_index + 1
                        ))
                    })
                })
                .collect::<std::result::Result<Vec<_>, _>>()?;
            rows.push(OdbcRow::new_shared(Arc::clone(&columns), values));
        }
    }

    Ok(rows)
}

fn build_buffer_bindings(
    cursor: &mut impl ResultSetMetadata,
    max_column_size: usize,
) -> std::result::Result<Vec<ColumnBinding>, sqlx_core::Error> {
    collect_columns(cursor).map(|columns| {
        columns
            .into_iter()
            .map(|column| ColumnBinding {
                buffer_desc: map_buffer_desc(column.type_info().data_type(), max_column_size),
                column,
            })
            .collect()
    })
}

fn map_buffer_desc(data_type: DataType, max_column_size: usize) -> BufferDesc {
    match data_type {
        DataType::TinyInt | DataType::SmallInt | DataType::Integer | DataType::BigInt => {
            BufferDesc::I64 { nullable: true }
        }
        DataType::Real => BufferDesc::F32 { nullable: true },
        DataType::Float { .. } | DataType::Double => BufferDesc::F64 { nullable: true },
        DataType::Bit => BufferDesc::Bit { nullable: true },
        DataType::Date => BufferDesc::Date { nullable: true },
        DataType::Time { .. } => BufferDesc::Time { nullable: true },
        DataType::Timestamp { .. } => BufferDesc::Timestamp { nullable: true },
        DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. } => {
            BufferDesc::Binary {
                max_bytes: max_column_size,
            }
        }
        DataType::Char { .. }
        | DataType::WChar { .. }
        | DataType::Varchar { .. }
        | DataType::WVarchar { .. }
        | DataType::LongVarchar { .. }
        | DataType::WLongVarchar { .. }
        | DataType::Other { .. }
        | DataType::Unknown
        | DataType::Decimal { .. }
        | DataType::Numeric { .. } => BufferDesc::Text {
            max_str_len: max_column_size,
        },
    }
}

fn buffered_column_values(
    slice: AnyColumnBufferSlice<'_>,
    binding: &ColumnBinding,
) -> std::result::Result<Vec<OdbcValueKind>, sqlx_core::Error> {
    let desc = binding.buffer_desc;
    Ok(match desc {
        BufferDesc::I8 { nullable } => buffered_numeric(&slice, desc, nullable, |value| {
            OdbcValueKind::TinyInt(value)
        })?,
        BufferDesc::I16 { nullable } => buffered_numeric(&slice, desc, nullable, |value| {
            OdbcValueKind::SmallInt(value)
        })?,
        BufferDesc::I32 { nullable } => buffered_numeric(&slice, desc, nullable, |value| {
            OdbcValueKind::Integer(value)
        })?,
        BufferDesc::I64 { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::BigInt)?
        }
        BufferDesc::U8 { nullable } => buffered_numeric(&slice, desc, nullable, |value: u8| {
            OdbcValueKind::BigInt(i64::from(value))
        })?,
        BufferDesc::F32 { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::Real)?
        }
        BufferDesc::F64 { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::Double)?
        }
        BufferDesc::Bit { nullable } => {
            buffered_numeric(&slice, desc, nullable, |value: odbc_api::Bit| {
                OdbcValueKind::Bit(value.as_bool())
            })?
        }
        BufferDesc::Date { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::Date)?
        }
        BufferDesc::Time { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::Time)?
        }
        BufferDesc::Timestamp { nullable } => {
            buffered_numeric(&slice, desc, nullable, OdbcValueKind::Timestamp)?
        }
        BufferDesc::Text { .. } => {
            let text = expect_buffer_slice(slice.as_text(), desc)?;
            text.iter()
                .map(|value| {
                    value
                        .map(|bytes| {
                            OdbcValueKind::Text(String::from_utf8_lossy(bytes).into_owned())
                        })
                        .unwrap_or(OdbcValueKind::Null)
                })
                .collect()
        }
        BufferDesc::WText { .. } => {
            let text = expect_buffer_slice(slice.as_wide_text(), desc)?;
            text.iter()
                .map(|value| {
                    value
                        .map(|chars| OdbcValueKind::Text(String::from_utf16_lossy(chars.into())))
                        .unwrap_or(OdbcValueKind::Null)
                })
                .collect()
        }
        BufferDesc::Binary { .. } => {
            let binary = expect_buffer_slice(slice.as_binary(), desc)?;
            binary
                .iter()
                .map(|value| {
                    value
                        .map(|bytes| OdbcValueKind::Binary(bytes.to_vec()))
                        .unwrap_or(OdbcValueKind::Null)
                })
                .collect()
        }
        BufferDesc::Numeric => {
            return Err(sqlx_core::Error::Protocol(format!(
                "unsupported ODBC buffer descriptor: {desc:?}"
            )))
        }
    })
}

fn buffered_numeric<T, F>(
    slice: &AnyColumnBufferSlice<'_>,
    desc: BufferDesc,
    nullable: bool,
    map: F,
) -> std::result::Result<Vec<OdbcValueKind>, sqlx_core::Error>
where
    T: Copy + odbc_api::Pod,
    F: FnMut(T) -> OdbcValueKind,
{
    if nullable {
        Ok(buffered_nullable_numeric(
            expect_buffer_slice(slice.as_nullable_slice::<T>(), desc)?,
            map,
        ))
    } else {
        Ok(expect_buffer_slice(slice.as_slice::<T>(), desc)?
            .iter()
            .copied()
            .map(map)
            .collect())
    }
}

fn buffered_nullable_numeric<T, F>(slice: NullableSlice<'_, T>, mut map: F) -> Vec<OdbcValueKind>
where
    T: Copy,
    F: FnMut(T) -> OdbcValueKind,
{
    slice
        .map(|value| value.copied().map(&mut map).unwrap_or(OdbcValueKind::Null))
        .collect()
}

fn expect_buffer_slice<T>(
    slice: Option<T>,
    desc: BufferDesc,
) -> std::result::Result<T, sqlx_core::Error> {
    slice.ok_or_else(|| {
        sqlx_core::Error::Protocol(format!(
            "ODBC column buffer {desc:?} did not match fetched slice"
        ))
    })
}

fn collect_rows_unbuffered<C>(mut cursor: C) -> std::result::Result<Vec<OdbcRow>, sqlx_core::Error>
where
    C: Cursor + ResultSetMetadata,
{
    let columns: Arc<[OdbcColumn]> = collect_columns(&mut cursor)?.into();
    let mut rows = Vec::new();

    while let Some(mut cursor_row) = cursor.next_row().map_err(|error| {
        crate::error::database_error_with_context(
            error,
            "ODBC unbuffered fetch failed while reading the next row",
        )
    })? {
        let mut values = Vec::with_capacity(columns.len());

        for column in columns.iter() {
            let column_number = u16::try_from(sqlx_core::column::Column::ordinal(column) + 1)
                .map_err(|_| {
                    sqlx_core::Error::Protocol("ODBC column index exceeds u16".to_owned())
                })?;
            values.push(fetch_value(&mut cursor_row, column_number, column)?);
        }

        rows.push(OdbcRow::new_shared(Arc::clone(&columns), values));
    }

    Ok(rows)
}

fn fetch_value(
    row: &mut odbc_api::CursorRow<'_>,
    column_number: u16,
    column: &OdbcColumn,
) -> std::result::Result<OdbcValue, sqlx_core::Error> {
    let data_type = column.type_info().data_type();

    let kind = match data_type {
        DataType::Bit => {
            let mut value = Nullable::<odbc_api::Bit>::null();
            row.get_data(column_number, &mut value).map_err(|error| {
                crate::error::database_error_with_context_lazy(error, || {
                    fetch_context(column, data_type)
                })
            })?;
            value
                .into_opt()
                .map(|value| OdbcValueKind::Bit(value.as_bool()))
                .unwrap_or(OdbcValueKind::Null)
        }
        DataType::TinyInt => fetch_nullable(
            row,
            column_number,
            column,
            data_type,
            OdbcValueKind::TinyInt,
        )?,
        DataType::SmallInt => fetch_nullable(
            row,
            column_number,
            column,
            data_type,
            OdbcValueKind::SmallInt,
        )?,
        DataType::Integer => fetch_nullable(
            row,
            column_number,
            column,
            data_type,
            OdbcValueKind::Integer,
        )?,
        DataType::BigInt => {
            fetch_nullable(row, column_number, column, data_type, OdbcValueKind::BigInt)?
        }
        DataType::Real => {
            fetch_nullable(row, column_number, column, data_type, OdbcValueKind::Real)?
        }
        DataType::Float { .. } | DataType::Double => {
            fetch_nullable(row, column_number, column, data_type, OdbcValueKind::Double)?
        }
        DataType::Date => {
            fetch_nullable(row, column_number, column, data_type, OdbcValueKind::Date)?
        }
        DataType::Time { .. } => {
            fetch_nullable(row, column_number, column, data_type, OdbcValueKind::Time)?
        }
        DataType::Timestamp { .. } => fetch_nullable(
            row,
            column_number,
            column,
            data_type,
            OdbcValueKind::Timestamp,
        )?,
        DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. } => {
            let mut value = Vec::new();
            if row.get_binary(column_number, &mut value).map_err(|error| {
                crate::error::database_error_with_context_lazy(error, || {
                    fetch_context(column, data_type)
                })
            })? {
                OdbcValueKind::Binary(value)
            } else {
                OdbcValueKind::Null
            }
        }
        _ => {
            let mut value = Vec::new();
            if row
                .get_wide_text(column_number, &mut value)
                .map_err(|error| {
                    crate::error::database_error_with_context_lazy(error, || {
                        fetch_context(column, data_type)
                    })
                })?
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
    column: &OdbcColumn,
    data_type: DataType,
    map: F,
) -> std::result::Result<OdbcValueKind, sqlx_core::Error>
where
    T: Default + Copy + odbc_api::parameter::CElement + odbc_api::handles::CDataMut,
    Nullable<T>: odbc_api::parameter::CElement + odbc_api::handles::CDataMut,
    F: FnOnce(T) -> OdbcValueKind,
{
    let mut value = Nullable::<T>::null();
    row.get_data(column_number, &mut value).map_err(|error| {
        crate::error::database_error_with_context_lazy(error, || fetch_context(column, data_type))
    })?;
    Ok(value.into_opt().map(map).unwrap_or(OdbcValueKind::Null))
}

fn fetch_context(column: &OdbcColumn, data_type: DataType) -> String {
    format!(
        "failed to fetch ODBC column {} (`{}`) as {data_type:?}",
        column.ordinal() + 1,
        column.name()
    )
}

fn sql_preview(sql: &str) -> String {
    const MAX_LEN: usize = 160;

    let compact = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    if compact.len() <= MAX_LEN {
        compact
    } else {
        let mut preview = compact.chars().take(MAX_LEN - 3).collect::<String>();
        preview.push_str("...");
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffered_fetch_maps_numeric_types_to_nullable_64_bit_buffers() {
        assert!(matches!(
            map_buffer_desc(DataType::TinyInt, 64),
            BufferDesc::I64 { nullable: true }
        ));
        assert!(matches!(
            map_buffer_desc(DataType::Integer, 64),
            BufferDesc::I64 { nullable: true }
        ));
        assert!(matches!(
            map_buffer_desc(DataType::BigInt, 64),
            BufferDesc::I64 { nullable: true }
        ));
    }

    #[test]
    fn buffered_fetch_uses_configured_limits_for_variable_sized_data() {
        assert_eq!(
            map_buffer_desc(DataType::Varchar { length: None }, 32),
            BufferDesc::Text { max_str_len: 32 }
        );
        assert_eq!(
            map_buffer_desc(DataType::Varbinary { length: None }, 16),
            BufferDesc::Binary { max_bytes: 16 }
        );
    }

    #[test]
    fn ping_query_uses_db2_dummy_table_for_db2_drivers() {
        assert_eq!(
            "SELECT 1 FROM SYSIBM.SYSDUMMY1",
            ping_query_for_dbms_name("DB2")
        );
        assert_eq!(
            "SELECT 1 FROM SYSIBM.SYSDUMMY1",
            ping_query_for_dbms_name("DB2 UDB for AS/400")
        );
        assert_eq!(
            "SELECT 1 FROM SYSIBM.SYSDUMMY1",
            ping_query_for_dbms_name("IBM DB2 for i")
        );
        assert_eq!(
            "SELECT 1 FROM SYSIBM.SYSDUMMY1",
            ping_query_for_dbms_name("iSeries")
        );
        assert_eq!(
            "SELECT 1 FROM SYSIBM.SYSDUMMY1",
            ping_query_for_dbms_name("IBM i")
        );
    }

    #[test]
    fn ping_query_keeps_select_one_for_non_db2_drivers() {
        assert_eq!("SELECT 1", ping_query_for_dbms_name("DuckDB"));
        assert_eq!("SELECT 1", ping_query_for_dbms_name("Microsoft SQL Server"));
        assert_eq!("SELECT 1", ping_query_for_dbms_name("PostgreSQL"));
    }
}
