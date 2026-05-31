use crate::value::OdbcValueRef;
use crate::{DataTypeExt, Odbc, OdbcArgumentValue, OdbcTypeInfo};
use bigdecimal::BigDecimal;
use odbc_api::DataType;
use sqlx_core::decode::Decode;
use sqlx_core::encode::{Encode, IsNull};
use sqlx_core::error::BoxDynError;
use sqlx_core::types::Type;
use std::str::FromStr;

impl Type<Odbc> for BigDecimal {
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

impl<'q> Encode<'q, Odbc> for BigDecimal {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.to_string()));
        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Odbc> for BigDecimal {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(text) = value.as_str() {
            return BigDecimal::from_str(text.trim())
                .map_err(|error| format!("bad decimal text: {error}").into());
        }

        if let Some(bytes) = value.as_bytes() {
            let text = std::str::from_utf8(bytes)?;
            return BigDecimal::from_str(text.trim())
                .map_err(|error| format!("bad decimal bytes: {error}").into());
        }

        if let Some(integer) = value.as_i64() {
            return Ok(BigDecimal::from(integer));
        }

        if let Some(float) = value.as_f64() {
            return Ok(BigDecimal::try_from(float)?);
        }

        Err("ODBC: cannot decode BigDecimal".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OdbcValue, OdbcValueKind};
    use sqlx_core::value::Value;

    #[test]
    fn bigdecimal_type_compatibility_matches_old_odbc() {
        assert!(<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::decimal(10, 2)
        ));
        assert!(<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::numeric(15, 4)
        ));
        assert!(<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::DOUBLE
        ));
        assert!(<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::float(24)
        ));
        assert!(<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::varchar(None)
        ));
        assert!(!<BigDecimal as Type<Odbc>>::compatible(
            &OdbcTypeInfo::varbinary(None)
        ));
    }

    #[test]
    fn bigdecimal_decodes_old_odbc_forms() -> Result<(), BoxDynError> {
        for (value, expected) in [
            (
                OdbcValue::new(OdbcValueKind::Text("123.456789".to_owned())),
                "123.456789",
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
                <BigDecimal as Decode<Odbc>>::decode(value.as_ref())?,
                BigDecimal::from_str(expected)?
            );
        }

        Ok(())
    }

    #[test]
    fn bigdecimal_encodes_as_text() -> Result<(), BoxDynError> {
        let mut buf = Vec::new();
        let decimal = BigDecimal::from_str("123.456")?;

        let result = <BigDecimal as Encode<Odbc>>::encode(decimal, &mut buf)?;

        assert!(matches!(result, IsNull::No));
        assert_eq!(buf, vec![OdbcArgumentValue::Text("123.456".to_owned())]);
        Ok(())
    }
}
