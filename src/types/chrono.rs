use crate::value::OdbcValueRef;
use crate::{DataTypeExt, Odbc, OdbcArgumentValue, OdbcTypeInfo, OdbcValueKind};
use ::chrono::{
    DateTime, Datelike, FixedOffset, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike, Utc,
};
use odbc_api::DataType;
use sqlx_core::decode::Decode;
use sqlx_core::encode::{Encode, IsNull};
use sqlx_core::error::BoxDynError;
use sqlx_core::types::Type;
use sqlx_core::value::ValueRef;

impl Type<Odbc> for NaiveDate {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::DATE
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        matches!(ty.data_type(), DataType::Date)
            || ty.data_type().accepts_character_data()
            || ty.data_type().accepts_numeric_data()
            || matches!(ty.data_type(), DataType::Other { .. } | DataType::Unknown)
    }
}

impl Type<Odbc> for NaiveTime {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::TIME
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        matches!(ty.data_type(), DataType::Time { .. })
            || ty.data_type().accepts_character_data()
            || ty.data_type().accepts_numeric_data()
            || matches!(ty.data_type(), DataType::Other { .. } | DataType::Unknown)
    }
}

impl Type<Odbc> for NaiveDateTime {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::TIMESTAMP
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        matches!(ty.data_type(), DataType::Timestamp { .. })
            || ty.data_type().accepts_character_data()
            || ty.data_type().accepts_numeric_data()
            || matches!(ty.data_type(), DataType::Other { .. } | DataType::Unknown)
    }
}

impl Type<Odbc> for DateTime<Utc> {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::TIMESTAMP
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        <NaiveDateTime as Type<Odbc>>::compatible(ty)
    }
}

impl Type<Odbc> for DateTime<FixedOffset> {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::TIMESTAMP
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        <NaiveDateTime as Type<Odbc>>::compatible(ty)
    }
}

impl Type<Odbc> for DateTime<Local> {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::TIMESTAMP
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        matches!(ty.data_type(), DataType::Timestamp { .. })
            || ty.data_type().accepts_character_data()
            || matches!(ty.data_type(), DataType::Other { .. } | DataType::Unknown)
    }
}

impl<'q> Encode<'q, Odbc> for NaiveDate {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Date(odbc_api::sys::Date {
            year: self.year() as i16,
            month: self.month() as u16,
            day: self.day() as u16,
        }));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Odbc> for NaiveTime {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Time(odbc_api::sys::Time {
            hour: self.hour() as u16,
            minute: self.minute() as u16,
            second: self.second() as u16,
        }));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Odbc> for NaiveDateTime {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Timestamp(timestamp_from_naive(*self)));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Odbc> for DateTime<Utc> {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.to_rfc3339()));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Odbc> for DateTime<FixedOffset> {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.to_rfc3339()));
        Ok(IsNull::No)
    }
}

impl<'q> Encode<'q, Odbc> for DateTime<Local> {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Timestamp(timestamp_from_naive(
            self.naive_local(),
        )));
        Ok(IsNull::No)
    }
}

fn timestamp_from_naive(value: NaiveDateTime) -> odbc_api::sys::Timestamp {
    odbc_api::sys::Timestamp {
        year: value.year() as i16,
        month: value.month() as u16,
        day: value.day() as u16,
        hour: value.hour() as u16,
        minute: value.minute() as u16,
        second: value.second() as u16,
        fraction: value.nanosecond(),
    }
}

fn parse_yyyymmdd_as_naive_date(value: i64) -> Option<NaiveDate> {
    if !(19000101..=30001231).contains(&value) {
        return None;
    }

    let year = (value / 10000) as i32;
    let month = ((value % 10000) / 100) as u32;
    let day = (value % 100) as u32;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn parse_yyyymmdd_text_as_naive_date(value: &str) -> Option<NaiveDate> {
    if value.len() != 8 || !value.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }

    let year = value[0..4].parse().ok()?;
    let month = value[4..6].parse().ok()?;
    let day = value[6..8].parse().ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
}

fn raw_date(value: OdbcValueRef<'_>) -> Option<odbc_api::sys::Date> {
    match ValueRef::to_owned(&value).kind() {
        OdbcValueKind::Date(date) => Some(*date),
        _ => None,
    }
}

fn raw_time(value: OdbcValueRef<'_>) -> Option<odbc_api::sys::Time> {
    match ValueRef::to_owned(&value).kind() {
        OdbcValueKind::Time(time) => Some(*time),
        _ => None,
    }
}

fn raw_timestamp(value: OdbcValueRef<'_>) -> Option<odbc_api::sys::Timestamp> {
    match ValueRef::to_owned(&value).kind() {
        OdbcValueKind::Timestamp(timestamp) => Some(*timestamp),
        _ => None,
    }
}

fn trimmed_text(value: OdbcValueRef<'_>) -> Option<String> {
    Some(value.as_str()?.trim_end_matches('\0').trim().to_owned())
}

fn naive_from_timestamp(value: odbc_api::sys::Timestamp) -> Result<NaiveDateTime, BoxDynError> {
    let date = NaiveDate::from_ymd_opt(value.year as i32, value.month as u32, value.day as u32)
        .ok_or_else(|| "ODBC: invalid date values in timestamp".to_string())?;
    let time = NaiveTime::from_hms_nano_opt(
        value.hour as u32,
        value.minute as u32,
        value.second as u32,
        value.fraction,
    )
    .ok_or_else(|| "ODBC: invalid time values in timestamp".to_string())?;
    Ok(NaiveDateTime::new(date, time))
}

impl<'r> Decode<'r, Odbc> for NaiveDate {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(date) = raw_date(value) {
            return NaiveDate::from_ymd_opt(date.year as i32, date.month as u32, date.day as u32)
                .ok_or_else(|| "ODBC: invalid date values".into());
        }

        if let Some(text) = trimmed_text(value) {
            if let Some(date) = parse_yyyymmdd_text_as_naive_date(&text) {
                return Ok(date);
            }

            if let Ok(date) = text.parse() {
                return Ok(date);
            }
        }

        if let Some(integer) = value.as_i64() {
            if let Some(date) = parse_yyyymmdd_as_naive_date(integer) {
                return Ok(date);
            }

            return Err(format!(
                "ODBC: cannot decode NaiveDate from integer '{integer}': not in YYYYMMDD range"
            )
            .into());
        }

        if let Some(float) = value.as_f64() {
            if let Some(date) = parse_yyyymmdd_as_naive_date(float as i64) {
                return Ok(date);
            }

            return Err(format!(
                "ODBC: cannot decode NaiveDate from float '{float}': not in YYYYMMDD range"
            )
            .into());
        }

        Err("ODBC: cannot decode NaiveDate".into())
    }
}

impl<'r> Decode<'r, Odbc> for NaiveTime {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(time) = raw_time(value) {
            return NaiveTime::from_hms_opt(
                time.hour as u32,
                time.minute as u32,
                time.second as u32,
            )
            .ok_or_else(|| "ODBC: invalid time values".into());
        }

        let Some(text) = trimmed_text(value) else {
            return Err("ODBC: cannot decode NaiveTime".into());
        };

        text.parse()
            .map_err(|error| format!("ODBC: cannot decode NaiveTime from '{text}': {error}").into())
    }
}

impl<'r> Decode<'r, Odbc> for NaiveDateTime {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(timestamp) = raw_timestamp(value) {
            return naive_from_timestamp(timestamp);
        }

        let Some(text) = trimmed_text(value) else {
            return Err("ODBC: cannot decode NaiveDateTime".into());
        };

        if let Ok(datetime) = NaiveDateTime::parse_from_str(&text, "%Y-%m-%d %H:%M:%S%.f") {
            return Ok(datetime);
        }

        if let Ok(datetime) = NaiveDateTime::parse_from_str(&text, "%Y-%m-%d %H:%M:%S") {
            return Ok(datetime);
        }

        text.parse().map_err(|error| {
            format!("ODBC: cannot decode NaiveDateTime from '{text}': {error}").into()
        })
    }
}

impl<'r> Decode<'r, Odbc> for DateTime<Utc> {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(timestamp) = raw_timestamp(value) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(
                naive_from_timestamp(timestamp)?,
                Utc,
            ));
        }

        let Some(text) = trimmed_text(value) else {
            return Err("ODBC: cannot decode DateTime<Utc>".into());
        };

        if let Ok(datetime) = text.parse() {
            return Ok(datetime);
        }

        if let Ok(datetime) = <NaiveDateTime as Decode<Odbc>>::decode(value) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc));
        }

        Err(format!("ODBC: cannot decode DateTime<Utc> from '{text}'").into())
    }
}

impl<'r> Decode<'r, Odbc> for DateTime<FixedOffset> {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(timestamp) = raw_timestamp(value) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(
                naive_from_timestamp(timestamp)?,
                Utc,
            )
            .fixed_offset());
        }

        let Some(text) = trimmed_text(value) else {
            return Err("ODBC: cannot decode DateTime<FixedOffset>".into());
        };

        if let Ok(datetime) = text.parse() {
            return Ok(datetime);
        }

        if let Ok(datetime) = <NaiveDateTime as Decode<Odbc>>::decode(value) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc).fixed_offset());
        }

        Err(format!("ODBC: cannot decode DateTime<FixedOffset> from '{text}'").into())
    }
}

impl<'r> Decode<'r, Odbc> for DateTime<Local> {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(timestamp) = raw_timestamp(value) {
            return Ok(DateTime::<Utc>::from_naive_utc_and_offset(
                naive_from_timestamp(timestamp)?,
                Utc,
            )
            .with_timezone(&Local));
        }

        Ok(<DateTime<Utc> as Decode<Odbc>>::decode(value)?.with_timezone(&Local))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OdbcValue;
    use sqlx_core::type_info::TypeInfo;
    use sqlx_core::value::Value;

    #[test]
    fn naive_date_type_compatibility_matches_old_odbc() {
        assert!(<NaiveDate as Type<Odbc>>::compatible(&OdbcTypeInfo::DATE));
        assert!(<NaiveDate as Type<Odbc>>::compatible(
            &OdbcTypeInfo::varchar(None)
        ));
        assert!(<NaiveDate as Type<Odbc>>::compatible(
            &OdbcTypeInfo::INTEGER
        ));
    }

    #[test]
    fn naive_date_decodes_old_text_and_numeric_forms() -> Result<(), BoxDynError> {
        for value in [
            OdbcValue::new(OdbcValueKind::Text("2020-01-02".to_owned())),
            OdbcValue::new(OdbcValueKind::Text("20200102".to_owned())),
            OdbcValue::new(OdbcValueKind::BigInt(20200102)),
            OdbcValue::new(OdbcValueKind::Double(20200102.0)),
        ] {
            assert_eq!(
                <NaiveDate as Decode<Odbc>>::decode(value.as_ref())?,
                NaiveDate::from_ymd_opt(2020, 1, 2).unwrap()
            );
        }

        Ok(())
    }

    #[test]
    fn chrono_values_decode_raw_odbc_temporal_kinds() -> Result<(), BoxDynError> {
        let date = OdbcValue::new(OdbcValueKind::Date(odbc_api::sys::Date {
            year: 2020,
            month: 1,
            day: 2,
        }));
        assert_eq!(
            <NaiveDate as Decode<Odbc>>::decode(date.as_ref())?,
            NaiveDate::from_ymd_opt(2020, 1, 2).unwrap()
        );

        let time = OdbcValue::new(OdbcValueKind::Time(odbc_api::sys::Time {
            hour: 15,
            minute: 30,
            second: 45,
        }));
        assert_eq!(
            <NaiveTime as Decode<Odbc>>::decode(time.as_ref())?,
            NaiveTime::from_hms_opt(15, 30, 45).unwrap()
        );

        let timestamp = OdbcValue::new(OdbcValueKind::Timestamp(odbc_api::sys::Timestamp {
            year: 2020,
            month: 1,
            day: 2,
            hour: 15,
            minute: 30,
            second: 45,
            fraction: 123_456_789,
        }));
        assert_eq!(
            <NaiveDateTime as Decode<Odbc>>::decode(timestamp.as_ref())?,
            NaiveDate::from_ymd_opt(2020, 1, 2)
                .unwrap()
                .and_hms_nano_opt(15, 30, 45, 123_456_789)
                .unwrap()
        );

        Ok(())
    }

    #[test]
    fn chrono_values_decode_text_datetime_forms() -> Result<(), BoxDynError> {
        let value = OdbcValue::new(OdbcValueKind::Text("2020-01-02 15:30:45".to_owned()));
        let expected = NaiveDate::from_ymd_opt(2020, 1, 2)
            .unwrap()
            .and_hms_opt(15, 30, 45)
            .unwrap();

        assert_eq!(
            <NaiveDateTime as Decode<Odbc>>::decode(value.as_ref())?,
            expected
        );
        assert_eq!(
            <DateTime<Utc> as Decode<Odbc>>::decode(value.as_ref())?,
            DateTime::<Utc>::from_naive_utc_and_offset(expected, Utc)
        );

        Ok(())
    }

    #[test]
    fn chrono_values_encode_to_old_odbc_argument_forms() -> Result<(), BoxDynError> {
        let mut buf = Vec::new();
        let date = NaiveDate::from_ymd_opt(2020, 1, 2).unwrap();
        let _ = <NaiveDate as Encode<Odbc>>::encode(date, &mut buf)?;
        assert_eq!(
            buf,
            vec![OdbcArgumentValue::Date(odbc_api::sys::Date {
                year: 2020,
                month: 1,
                day: 2
            })]
        );

        buf.clear();
        let datetime = date.and_hms_nano_opt(15, 30, 45, 123_456_789).unwrap();
        let _ = <NaiveDateTime as Encode<Odbc>>::encode(datetime, &mut buf)?;
        assert_eq!(
            buf,
            vec![OdbcArgumentValue::Timestamp(odbc_api::sys::Timestamp {
                year: 2020,
                month: 1,
                day: 2,
                hour: 15,
                minute: 30,
                second: 45,
                fraction: 123_456_789
            })]
        );

        buf.clear();
        let utc = DateTime::<Utc>::from_naive_utc_and_offset(datetime, Utc);
        let _ = <DateTime<Utc> as Encode<Odbc>>::encode(utc, &mut buf)?;
        assert_eq!(buf, vec![OdbcArgumentValue::Text(utc.to_rfc3339())]);

        Ok(())
    }

    #[test]
    fn chrono_type_info_names_match_old_odbc() {
        assert_eq!(<NaiveDate as Type<Odbc>>::type_info().name(), "DATE");
        assert_eq!(<NaiveTime as Type<Odbc>>::type_info().name(), "TIME");
        assert_eq!(
            <NaiveDateTime as Type<Odbc>>::type_info().name(),
            "TIMESTAMP"
        );
        assert_eq!(
            <DateTime<FixedOffset> as Type<Odbc>>::type_info().name(),
            "TIMESTAMP"
        );
    }
}
