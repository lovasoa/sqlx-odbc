//! Runtime `Any` driver support for ODBC.

use crate::{
    DataTypeExt, Odbc, OdbcArgumentValue, OdbcArguments, OdbcColumn, OdbcConnectOptions,
    OdbcConnection, OdbcQueryResult, OdbcTransactionManager, OdbcTypeInfo,
};
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
use futures_util::{future, stream, FutureExt, StreamExt};
use sqlx_core::any::driver::AnyDriver;
use sqlx_core::any::{
    AnyArguments, AnyColumn, AnyConnectOptions, AnyConnectionBackend, AnyQueryResult, AnyRow,
    AnyStatement, AnyTypeInfo, AnyTypeInfoKind, AnyValueKind,
};
use sqlx_core::column::Column;
use sqlx_core::connection::{ConnectOptions, Connection};
use sqlx_core::database::Database;
use sqlx_core::ext::ustr::UStr;
use sqlx_core::row::Row;
use sqlx_core::sql_str::SqlStr;
use sqlx_core::statement::Statement;
use sqlx_core::transaction::TransactionManager;
use sqlx_core::{Either, HashMap};
use std::sync::Arc;

/// Installable ODBC driver for SQLx `Any` connections.
pub const DRIVER: AnyDriver = AnyDriver::without_migrate::<Odbc>();

impl AnyConnectionBackend for OdbcConnection {
    fn name(&self) -> &str {
        <Odbc as Database>::NAME
    }

    fn close(self: Box<Self>) -> BoxFuture<'static, sqlx_core::Result<()>> {
        Connection::close(*self).boxed()
    }

    fn close_hard(self: Box<Self>) -> BoxFuture<'static, sqlx_core::Result<()>> {
        Connection::close_hard(*self).boxed()
    }

    fn ping(&mut self) -> BoxFuture<'_, sqlx_core::Result<()>> {
        Connection::ping(self).boxed()
    }

    fn begin(&mut self, statement: Option<SqlStr>) -> BoxFuture<'_, sqlx_core::Result<()>> {
        OdbcTransactionManager::begin(self, statement).boxed()
    }

    fn commit(&mut self) -> BoxFuture<'_, sqlx_core::Result<()>> {
        OdbcTransactionManager::commit(self).boxed()
    }

    fn rollback(&mut self) -> BoxFuture<'_, sqlx_core::Result<()>> {
        OdbcTransactionManager::rollback(self).boxed()
    }

    fn start_rollback(&mut self) {
        OdbcTransactionManager::start_rollback(self);
    }

    fn get_transaction_depth(&self) -> usize {
        OdbcTransactionManager::get_transaction_depth(self)
    }

    fn shrink_buffers(&mut self) {
        Connection::shrink_buffers(self);
    }

    fn flush(&mut self) -> BoxFuture<'_, sqlx_core::Result<()>> {
        Connection::flush(self).boxed()
    }

    fn should_flush(&self) -> bool {
        Connection::should_flush(self)
    }

    fn fetch_many(
        &mut self,
        query: SqlStr,
        persistent: bool,
        arguments: Option<AnyArguments>,
    ) -> BoxStream<'_, sqlx_core::Result<Either<AnyQueryResult, AnyRow>>> {
        let arguments = match arguments.map(map_arguments).transpose() {
            Ok(arguments) => arguments,
            Err(error) => return stream::once(future::ready(Err(error))).boxed(),
        };
        let rx = self.execute_receiver(query, persistent, arguments);

        stream::unfold(rx, |rx| async move {
            let item = rx.recv_async().await.ok()?;
            Some((item, rx))
        })
        .map(|item| match item? {
            Either::Left(result) => Ok(Either::Left(map_result(result))),
            Either::Right(row) => {
                let column_names = column_names(row.columns());
                AnyRow::map_from(&row, column_names).map(Either::Right)
            }
        })
        .boxed()
    }

    fn fetch_optional(
        &mut self,
        query: SqlStr,
        persistent: bool,
        arguments: Option<AnyArguments>,
    ) -> BoxFuture<'_, sqlx_core::Result<Option<AnyRow>>> {
        let arguments = arguments.map(map_arguments).transpose();

        Box::pin(async move {
            let arguments = arguments?;
            let rx = self.execute_receiver(query, persistent, arguments);
            while let Ok(item) = rx.recv_async().await {
                match item? {
                    Either::Left(_) => {}
                    Either::Right(row) => {
                        let column_names = column_names(row.columns());
                        return AnyRow::map_from(&row, column_names).map(Some);
                    }
                }
            }
            Ok(None)
        })
    }

    fn prepare_with<'c, 'q: 'c>(
        &'c mut self,
        sql: SqlStr,
        _parameters: &[AnyTypeInfo],
    ) -> BoxFuture<'c, sqlx_core::Result<AnyStatement>> {
        Box::pin(async move {
            let statement = self.prepare_blocking(sql)?;
            let column_names = column_names(statement.columns());
            AnyStatement::try_from_statement(statement, column_names)
        })
    }
}

impl<'a> TryFrom<&'a AnyConnectOptions> for OdbcConnectOptions {
    type Error = sqlx_core::Error;

    fn try_from(options: &'a AnyConnectOptions) -> Result<Self, Self::Error> {
        let mut options_out = OdbcConnectOptions::from_url(&options.database_url)?;
        options_out.log_statements = options.log_settings.statements_level;
        options_out.log_slow_statements = options.log_settings.slow_statements_level;
        options_out.log_slow_statement_duration = options.log_settings.slow_statements_duration;
        Ok(options_out)
    }
}

impl<'a> TryFrom<&'a OdbcTypeInfo> for AnyTypeInfo {
    type Error = sqlx_core::Error;

    fn try_from(type_info: &'a OdbcTypeInfo) -> Result<Self, Self::Error> {
        let kind = match type_info.data_type() {
            odbc_api::DataType::Unknown => AnyTypeInfoKind::Null,
            odbc_api::DataType::Bit => AnyTypeInfoKind::Bool,
            odbc_api::DataType::TinyInt | odbc_api::DataType::SmallInt => AnyTypeInfoKind::SmallInt,
            odbc_api::DataType::Integer => AnyTypeInfoKind::Integer,
            odbc_api::DataType::BigInt => AnyTypeInfoKind::BigInt,
            odbc_api::DataType::Real => AnyTypeInfoKind::Real,
            odbc_api::DataType::Float { .. } | odbc_api::DataType::Double => {
                AnyTypeInfoKind::Double
            }
            data_type if data_type.accepts_character_data() => AnyTypeInfoKind::Text,
            data_type if data_type.accepts_binary_data() => AnyTypeInfoKind::Blob,
            data_type => {
                return Err(sqlx_core::Error::AnyDriverError(
                    format!(
                        "ODBC Any conversion does not support result column type {data_type:?}"
                    )
                    .into(),
                ));
            }
        };

        Ok(AnyTypeInfo { kind })
    }
}

impl<'a> TryFrom<&'a OdbcColumn> for AnyColumn {
    type Error = sqlx_core::Error;

    fn try_from(column: &'a OdbcColumn) -> Result<Self, Self::Error> {
        let type_info = AnyTypeInfo::try_from(column.type_info()).map_err(|error| {
            sqlx_core::Error::ColumnDecode {
                index: column.name().to_owned(),
                source: error.into(),
            }
        })?;

        Ok(Self {
            ordinal: column.ordinal(),
            name: UStr::new(column.name()),
            type_info,
        })
    }
}

fn map_arguments(arguments: AnyArguments) -> sqlx_core::Result<OdbcArguments> {
    let mut out = OdbcArguments::default();

    for value in arguments.values.0 {
        out.add_value(match value {
            AnyValueKind::Null(kind) => OdbcArgumentValue::Null(any_type_to_odbc(kind)),
            AnyValueKind::Bool(value) => OdbcArgumentValue::Bit(value),
            AnyValueKind::SmallInt(value) => OdbcArgumentValue::Int(i64::from(value)),
            AnyValueKind::Integer(value) => OdbcArgumentValue::Int(i64::from(value)),
            AnyValueKind::BigInt(value) => OdbcArgumentValue::Int(value),
            AnyValueKind::Real(value) => OdbcArgumentValue::Float(f64::from(value)),
            AnyValueKind::Double(value) => OdbcArgumentValue::Float(value),
            AnyValueKind::Text(value) => OdbcArgumentValue::Text(value.to_string()),
            AnyValueKind::TextSlice(value) => OdbcArgumentValue::Text(value.to_string()),
            AnyValueKind::Blob(value) => OdbcArgumentValue::Bytes(value.to_vec()),
            other => {
                return Err(sqlx_core::Error::AnyDriverError(
                    format!("ODBC Any arguments do not support value kind {other:?}").into(),
                ))
            }
        });
    }

    Ok(out)
}

fn any_type_to_odbc(kind: AnyTypeInfoKind) -> OdbcTypeInfo {
    OdbcTypeInfo::new(match kind {
        AnyTypeInfoKind::Null => odbc_api::DataType::Unknown,
        AnyTypeInfoKind::Bool => odbc_api::DataType::Bit,
        AnyTypeInfoKind::SmallInt => odbc_api::DataType::SmallInt,
        AnyTypeInfoKind::Integer => odbc_api::DataType::Integer,
        AnyTypeInfoKind::BigInt => odbc_api::DataType::BigInt,
        AnyTypeInfoKind::Real => odbc_api::DataType::Real,
        AnyTypeInfoKind::Double => odbc_api::DataType::Double,
        AnyTypeInfoKind::Text => odbc_api::DataType::WVarchar { length: None },
        AnyTypeInfoKind::Blob => odbc_api::DataType::Varbinary { length: None },
    })
}

fn map_result(result: OdbcQueryResult) -> AnyQueryResult {
    AnyQueryResult {
        rows_affected: result.rows_affected(),
        last_insert_id: None,
    }
}

fn column_names(columns: &[OdbcColumn]) -> Arc<HashMap<UStr, usize>> {
    Arc::new(
        columns
            .iter()
            .map(|column| (UStr::new(column.name()), column.ordinal()))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_stable_odbc_types_to_any_types() {
        assert_eq!(
            AnyTypeInfo::try_from(&OdbcTypeInfo::new(odbc_api::DataType::Bit))
                .unwrap()
                .kind(),
            AnyTypeInfoKind::Bool
        );
        assert_eq!(
            AnyTypeInfo::try_from(&OdbcTypeInfo::new(odbc_api::DataType::Integer))
                .unwrap()
                .kind(),
            AnyTypeInfoKind::Integer
        );
        assert_eq!(
            AnyTypeInfo::try_from(&OdbcTypeInfo::new(odbc_api::DataType::WVarchar {
                length: None
            }))
            .unwrap()
            .kind(),
            AnyTypeInfoKind::Text
        );
        assert_eq!(
            AnyTypeInfo::try_from(&OdbcTypeInfo::new(odbc_api::DataType::Varbinary {
                length: None
            }))
            .unwrap()
            .kind(),
            AnyTypeInfoKind::Blob
        );
    }

    #[test]
    fn rejects_unstable_odbc_types_for_any_mapping() {
        assert!(matches!(
            AnyTypeInfo::try_from(&OdbcTypeInfo::new(odbc_api::DataType::Timestamp {
                precision: 6
            })),
            Err(sqlx_core::Error::AnyDriverError(_))
        ));
    }
}
