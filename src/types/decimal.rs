use crate::value::OdbcValueRef;
use crate::{DataTypeExt, Odbc, OdbcArgumentValue, OdbcTypeInfo};
use odbc_api::DataType;
use rust_decimal::Decimal;
use sqlx_core::decode::Decode;
use sqlx_core::encode::{Encode, IsNull};
use sqlx_core::error::BoxDynError;
use sqlx_core::types::Type;
use std::str::FromStr;

impl Type<Odbc> for Decimal {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::numeric(28, 4)
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        matches!(
            ty.data_type(),
            DataType::Numeric { .. }
                | DataType::Decimal { .. }
                | DataType::Double
                | DataType::Float { .. }
        ) || ty.data_type().accepts_character_data()
    }
}

impl<'q> Encode<'q, Odbc> for Decimal {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.to_string()));
        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Odbc> for Decimal {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(text) = value.as_str() {
            return Ok(Decimal::from_str(text.trim())?);
        }

        if let Some(bytes) = value.as_bytes() {
            let text = std::str::from_utf8(bytes)?;
            return Ok(Decimal::from_str(text.trim())?);
        }

        if let Some(integer) = value.as_i64() {
            return Ok(Decimal::from(integer));
        }

        if let Some(float) = value.as_f64() {
            if let Ok(decimal) = Decimal::try_from(float) {
                return Ok(decimal);
            }
        }

        Err("ODBC: cannot decode Decimal".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OdbcValue, OdbcValueKind};
    use sqlx_core::value::Value;

    #[test]
    fn decimal_type_compatibility_matches_old_odbc() {
        assert!(<Decimal as Type<Odbc>>::compatible(&OdbcTypeInfo::decimal(
            10, 2
        )));
        assert!(<Decimal as Type<Odbc>>::compatible(&OdbcTypeInfo::numeric(
            15, 4
        )));
        assert!(<Decimal as Type<Odbc>>::compatible(&OdbcTypeInfo::DOUBLE));
        assert!(<Decimal as Type<Odbc>>::compatible(&OdbcTypeInfo::float(
            24
        )));
        assert!(<Decimal as Type<Odbc>>::compatible(&OdbcTypeInfo::varchar(
            None
        )));
        assert!(!<Decimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::varbinary(None)
        ));
    }

    #[test]
    fn decimal_decodes_old_odbc_forms() -> Result<(), BoxDynError> {
        for (value, expected) in [
            (
                OdbcValue::new(OdbcValueKind::Text("123.456".to_owned())),
                "123.456",
            ),
            (
                OdbcValue::new(OdbcValueKind::Text("  987.654  ".to_owned())),
                "987.654",
            ),
            (
                OdbcValue::new(OdbcValueKind::Binary(b"-123.456".to_vec())),
                "-123.456",
            ),
            (OdbcValue::new(OdbcValueKind::BigInt(42)), "42"),
        ] {
            assert_eq!(
                <Decimal as Decode<Odbc>>::decode(value.as_ref())?,
                Decimal::from_str(expected)?
            );
        }

        Ok(())
    }

    #[test]
    fn decimal_decodes_float_with_expected_tolerance() -> Result<(), BoxDynError> {
        let value = OdbcValue::new(OdbcValueKind::Double(123.456));
        let decoded = <Decimal as Decode<Odbc>>::decode(value.as_ref())?;
        let expected = Decimal::from_str("123.456")?;

        assert!((decoded - expected).abs() < Decimal::from_str("0.001")?);
        Ok(())
    }

    #[test]
    fn decimal_encodes_as_text() -> Result<(), BoxDynError> {
        let mut buf = Vec::new();
        let decimal = Decimal::from_str("123.456")?;

        let result = <Decimal as Encode<Odbc>>::encode(decimal, &mut buf)?;

        assert!(matches!(result, IsNull::No));
        assert_eq!(buf, vec![OdbcArgumentValue::Text("123.456".to_owned())]);
        Ok(())
    }

    #[test]
    fn decimal_type_info_name_matches_old_odbc() {
        use sqlx_core::type_info::TypeInfo;

        assert_eq!(<Decimal as Type<Odbc>>::type_info().name(), "NUMERIC");
    }
}
