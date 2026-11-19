use super::get_test_conn;
use sqlx_core::{connection::Connection, query::query, row::Row};

#[cfg(feature = "bigdecimal")]
#[tokio::test]
async fn bigdecimal_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use bigdecimal::BigDecimal;

    let Some(mut conn) = get_test_conn("BigDecimal round trip").await? else {
        return Ok(());
    };

    let value: BigDecimal = "12345678901234.123456".parse()?;
    let row = query("SELECT CAST(? AS DECIMAL(20, 6))")
        .bind(&value)
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<BigDecimal, _>(0)?, value);
    conn.close().await?;
    Ok(())
}

#[cfg(feature = "rust_decimal")]
#[tokio::test]
async fn rust_decimal_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use rust_decimal::Decimal;

    let Some(mut conn) = get_test_conn("rust_decimal round trip").await? else {
        return Ok(());
    };

    let value: Decimal = "-98765432101234.654321".parse()?;
    let row = query("SELECT CAST(? AS DECIMAL(20, 6))")
        .bind(value)
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<Decimal, _>(0)?, value);
    conn.close().await?;
    Ok(())
}

#[cfg(feature = "chrono")]
#[tokio::test]
async fn chrono_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};

    let Some(mut conn) = get_test_conn("chrono round trip").await? else {
        return Ok(());
    };

    let date = NaiveDate::from_ymd_opt(2024, 2, 29).unwrap();
    let time = NaiveTime::from_hms_opt(15, 30, 45).unwrap();
    // The DuckDB ODBC driver does not round-trip timestamp fractions.
    let timestamp = date.and_time(time);
    let row = query("SELECT CAST(? AS DATE), CAST(? AS TIME), CAST(? AS TIMESTAMP)")
        .bind(date)
        .bind(time)
        .bind(timestamp)
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<NaiveDate, _>(0)?, date);
    assert_eq!(row.try_get::<NaiveTime, _>(1)?, time);
    assert_eq!(row.try_get::<NaiveDateTime, _>(2)?, timestamp);
    conn.close().await?;
    Ok(())
}

#[cfg(feature = "time")]
#[tokio::test]
async fn time_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use time::{
        macros::{date, datetime, time},
        Date, PrimitiveDateTime, Time,
    };

    let Some(mut conn) = get_test_conn("time round trip").await? else {
        return Ok(());
    };

    let date = date!(2024 - 02 - 29);
    let time = time!(15:30:45);
    let timestamp = datetime!(2024-02-29 15:30:45);
    let row = query("SELECT CAST(? AS DATE), CAST(? AS TIME), CAST(? AS TIMESTAMP)")
        .bind(date)
        .bind(time)
        .bind(timestamp)
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<Date, _>(0)?, date);
    assert_eq!(row.try_get::<Time, _>(1)?, time);
    assert_eq!(row.try_get::<PrimitiveDateTime, _>(2)?, timestamp);
    conn.close().await?;
    Ok(())
}

#[cfg(feature = "json")]
#[tokio::test]
async fn json_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use serde_json::{json, Value};
    use sqlx_core::types::Json;

    let Some(mut conn) = get_test_conn("JSON round trip").await? else {
        return Ok(());
    };

    let value = json!({"name": "café", "values": [true, null, 42]});
    let row = query("SELECT CAST(? AS JSON)")
        .bind(Json(&value))
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<Json<Value>, _>(0)?.0, value);
    conn.close().await?;
    Ok(())
}

#[cfg(feature = "uuid")]
#[tokio::test]
async fn uuid_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    use uuid::Uuid;

    let Some(mut conn) = get_test_conn("UUID round trip").await? else {
        return Ok(());
    };

    let value = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;
    let row = query("SELECT CAST(? AS UUID)")
        .bind(value)
        .fetch_one(&mut conn)
        .await?;

    assert_eq!(row.try_get::<Uuid, _>(0)?, value);
    conn.close().await?;
    Ok(())
}
