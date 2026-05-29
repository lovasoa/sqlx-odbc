use futures_util::TryStreamExt;
use sqlx_core::connection::{ConnectOptions, Connection};
use sqlx_core::executor::Executor;
use sqlx_core::row::Row;
use sqlx_core::statement::Statement;
use sqlx_core::value::ValueRef;
use sqlx_core::Either;
use sqlx_odbc::{OdbcConnectOptions, OdbcConnection};
use std::str::FromStr;
use std::sync::Once;

static ANY_DRIVERS: &[sqlx_core::any::driver::AnyDriver] = &[sqlx_odbc::any::DRIVER];

fn database_url(test_name: &str) -> Option<String> {
    match std::env::var("ODBC_DATABASE_URL") {
        Ok(value) if !value.trim().is_empty() => Some(value),
        _ => {
            eprintln!("skipping {test_name}: ODBC_DATABASE_URL is not set");
            None
        }
    }
}

fn get_blocking_test_conn(
    test_name: &str,
) -> Result<Option<OdbcConnection>, Box<dyn std::error::Error>> {
    let Some(url) = database_url(test_name) else {
        return Ok(None);
    };

    let options = OdbcConnectOptions::from_str(&url)?;
    Ok(Some(options.connect_blocking()?))
}

async fn get_test_conn(
    test_name: &str,
) -> Result<Option<OdbcConnection>, Box<dyn std::error::Error>> {
    let Some(url) = database_url(test_name) else {
        return Ok(None);
    };

    Ok(Some(OdbcConnection::connect(&url).await?))
}

async fn get_test_conn_with<F>(
    test_name: &str,
    configure: F,
) -> Result<Option<OdbcConnection>, Box<dyn std::error::Error>>
where
    F: FnOnce(&mut OdbcConnectOptions),
{
    let Some(url) = database_url(test_name) else {
        return Ok(None);
    };

    let mut options = OdbcConnectOptions::from_str(&url)?;
    configure(&mut options);

    Ok(Some(options.connect().await?))
}

fn any_database_url(test_name: &str) -> Option<String> {
    database_url(test_name).map(|url| {
        if url.starts_with("odbc:") {
            url
        } else {
            format!("odbc:{url}")
        }
    })
}

async fn get_any_test_conn(
    test_name: &str,
) -> Result<Option<sqlx_core::any::AnyConnection>, Box<dyn std::error::Error>> {
    static INSTALL: Once = Once::new();

    let Some(url) = any_database_url(test_name) else {
        return Ok(None);
    };

    INSTALL.call_once(|| {
        sqlx_core::any::driver::install_drivers(ANY_DRIVERS)
            .expect("ODBC Any driver should install once");
    });

    Ok(Some(sqlx_core::any::AnyConnection::connect(&url).await?))
}

#[test]
fn integration_connection_string_forms_parse() {
    let dsn = OdbcConnectOptions::from_str("ExampleDsn").unwrap();
    assert_eq!(dsn.connection_string(), "DSN=ExampleDsn");

    let conn_str = OdbcConnectOptions::from_str("DSN=ExampleDsn;UID=user").unwrap();
    assert_eq!(conn_str.connection_string(), "DSN=ExampleDsn;UID=user");

    let legacy = OdbcConnectOptions::from_str("odbc:DSN=ExampleDsn").unwrap();
    assert_eq!(legacy.connection_string(), "DSN=ExampleDsn");
}

#[test]
fn connect_and_ping_when_configured() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_blocking_test_conn("ODBC blocking connection test")? else {
        return Ok(());
    };

    conn.ping_blocking()?;
    let _dbms_name = conn.dbms_name()?;

    Ok(())
}

#[tokio::test]
async fn sqlx_connection_connect_ping_and_transaction_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx connection test").await? else {
        return Ok(());
    };

    conn.ping().await?;

    let tx = conn.begin().await?;
    tx.rollback().await?;

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_fetches_basic_row_when_configured() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx row fetch test").await? else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT 1")
        .fetch_one(&mut conn)
        .await?;
    let value = ValueRef::to_owned(&row.try_get_raw(0)?);
    assert_eq!(value.as_i64(), Some(1));

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_fetch_many_ends_rows_with_query_result_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx fetch_many result test").await? else {
        return Ok(());
    };

    let results: Vec<_> = (&mut conn)
        .fetch_many(sqlx_core::query::query("SELECT 1"))
        .try_collect()
        .await?;

    assert_eq!(results.len(), 2);
    let Either::Right(row) = &results[0] else {
        panic!("first fetch_many item should be a row");
    };
    assert_eq!(row.try_get::<i32, _>(0)?, 1);
    let Either::Left(result) = &results[1] else {
        panic!("last fetch_many item should be a query result");
    };
    assert_eq!(result.rows_affected(), 0);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_fetches_basic_row_in_buffered_mode_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn_with("ODBC SQLx buffered row fetch test", |options| {
        options.batch_size(2).max_column_size(Some(64));
    })
    .await?
    else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT 1")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.try_get::<i32, _>(0)?, 1);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_decodes_decimal_integer_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx decimal integer decode test").await? else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT CAST(42 AS DECIMAL(10, 0))")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.try_get::<i32, _>(0)?, 42);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_decodes_decimal_integer_in_buffered_mode_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn_with("ODBC SQLx buffered decimal decode test", |options| {
        options.batch_size(2).max_column_size(Some(64));
    })
    .await?
    else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT CAST(42 AS DECIMAL(10, 0))")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.try_get::<i32, _>(0)?, 42);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_binds_parameter_when_configured() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx parameter binding test").await? else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT CAST(? AS INTEGER)")
        .bind(7_i32)
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.try_get::<i32, _>(0)?, 7);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_binds_heterogeneous_parameters_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx heterogeneous parameter binding test").await?
    else {
        return Ok(());
    };

    let row = sqlx_core::query::query(
        "SELECT CAST(? AS INTEGER), CAST(? AS VARCHAR(32)), CAST(? AS DOUBLE)",
    )
    .bind(7_i32)
    .bind("odbc-param")
    .bind(2.5_f64)
    .fetch_one(&mut conn)
    .await?;

    assert_eq!(row.try_get::<i32, _>(0)?, 7);
    assert_eq!(row.try_get::<String, _>(1)?.trim_end(), "odbc-param");
    assert_eq!(row.try_get::<f64, _>(2)?, 2.5);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_query_binds_typed_null_when_configured() -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx typed null binding test").await? else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT CAST(? AS INTEGER)")
        .bind(Option::<i32>::None)
        .fetch_one(&mut conn)
        .await?;
    assert!(row.try_get_raw(0)?.is_null());

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn sqlx_prepare_reports_basic_metadata_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_test_conn("ODBC SQLx prepare metadata test").await? else {
        return Ok(());
    };

    let statement = (&mut conn)
        .prepare(sqlx_core::sql_str::SqlStr::from_static(
            "SELECT 1 AS answer",
        ))
        .await?;

    assert_eq!(statement.parameters(), Some(sqlx_core::Either::Right(0)));
    assert_eq!(statement.columns().len(), 1);
    assert_eq!(
        sqlx_core::column::Column::name(&statement.columns()[0]),
        "answer"
    );

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn any_connection_fetches_basic_row_when_configured() -> Result<(), Box<dyn std::error::Error>>
{
    let Some(mut conn) = get_any_test_conn("ODBC Any row fetch test").await? else {
        return Ok(());
    };

    let row = sqlx_core::query::query("SELECT 1")
        .fetch_one(&mut conn)
        .await?;
    assert_eq!(row.try_get::<i32, _>(0)?, 1);

    conn.close().await?;
    Ok(())
}

#[tokio::test]
async fn any_fetch_many_ends_rows_with_query_result_when_configured(
) -> Result<(), Box<dyn std::error::Error>> {
    let Some(mut conn) = get_any_test_conn("ODBC Any fetch_many result test").await? else {
        return Ok(());
    };

    let results: Vec<_> = (&mut conn)
        .fetch_many(sqlx_core::query::query("SELECT 1"))
        .try_collect()
        .await?;

    assert_eq!(results.len(), 2);
    let Either::Right(row) = &results[0] else {
        panic!("first Any fetch_many item should be a row");
    };
    assert_eq!(row.try_get::<i32, _>(0)?, 1);
    let Either::Left(result) = &results[1] else {
        panic!("last Any fetch_many item should be a query result");
    };
    assert_eq!(result.rows_affected(), 0);

    conn.close().await?;
    Ok(())
}
