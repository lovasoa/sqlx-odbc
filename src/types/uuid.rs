use crate::value::OdbcValueRef;
use crate::{DataTypeExt, Odbc, OdbcArgumentValue, OdbcTypeInfo};
use sqlx_core::decode::Decode;
use sqlx_core::encode::{Encode, IsNull};
use sqlx_core::error::BoxDynError;
use sqlx_core::types::Type;
use std::num::NonZeroUsize;
use uuid::Uuid;

impl Type<Odbc> for Uuid {
    fn type_info() -> OdbcTypeInfo {
        OdbcTypeInfo::varchar(NonZeroUsize::new(36))
    }

    fn compatible(ty: &OdbcTypeInfo) -> bool {
        ty.data_type().accepts_character_data()
            || ty.data_type().accepts_binary_data()
            || matches!(
                ty.data_type(),
                odbc_api::DataType::Other { .. } | odbc_api::DataType::Unknown
            )
    }
}

impl<'q> Encode<'q, Odbc> for Uuid {
    fn encode_by_ref(&self, buf: &mut Vec<OdbcArgumentValue>) -> Result<IsNull, BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.to_string()));
        Ok(IsNull::No)
    }
}

impl<'r> Decode<'r, Odbc> for Uuid {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, BoxDynError> {
        if let Some(bytes) = value.as_bytes() {
            if bytes.len() == 16 {
                return Ok(Uuid::from_slice(bytes)?);
            }

            if let Ok(text) = std::str::from_utf8(bytes) {
                if let Ok(uuid) = Uuid::parse_str(text.trim()) {
                    return Ok(uuid);
                }
            }
        }

        if let Some(text) = value.as_str() {
            if let Ok(uuid) = Uuid::parse_str(text.trim()) {
                return Ok(uuid);
            }
        }

        Err("ODBC: cannot decode Uuid".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OdbcValue, OdbcValueKind};
    use sqlx_core::value::Value;

    #[test]
    fn uuid_type_compatibility_matches_old_odbc() {
        assert!(<Uuid as Type<Odbc>>::compatible(&OdbcTypeInfo::varchar(
            None
        )));
        assert!(<Uuid as Type<Odbc>>::compatible(&OdbcTypeInfo::varbinary(
            None
        )));
        assert!(!<Uuid as Type<Odbc>>::compatible(&OdbcTypeInfo::INTEGER));
    }

    #[test]
    fn uuid_decodes_text_and_binary_forms() -> Result<(), BoxDynError> {
        let uuid = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;

        for value in [
            OdbcValue::new(OdbcValueKind::Text(format!(" {uuid} "))),
            OdbcValue::new(OdbcValueKind::Binary(uuid.as_bytes().to_vec())),
            OdbcValue::new(OdbcValueKind::Binary(uuid.to_string().into_bytes())),
        ] {
            assert_eq!(<Uuid as Decode<Odbc>>::decode(value.as_ref())?, uuid);
        }

        Ok(())
    }

    #[test]
    fn uuid_encodes_as_text() -> Result<(), BoxDynError> {
        let mut buf = Vec::new();
        let uuid = Uuid::nil();

        let result = <Uuid as Encode<Odbc>>::encode(uuid, &mut buf)?;

        assert!(matches!(result, IsNull::No));
        assert_eq!(
            buf,
            vec![OdbcArgumentValue::Text(
                "00000000-0000-0000-0000-000000000000".to_owned()
            )]
        );
        Ok(())
    }
}
