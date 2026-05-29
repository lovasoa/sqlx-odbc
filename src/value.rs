use std::borrow::Cow;

/// A small owned ODBC value representation used by unit tests and Any-mapping work.
#[derive(Debug, Clone, PartialEq)]
pub struct OdbcValue {
    kind: OdbcValueKind,
}

impl OdbcValue {
    /// Creates a new value from a raw kind.
    pub fn new(kind: OdbcValueKind) -> Self {
        Self { kind }
    }

    /// Returns the raw value kind.
    pub fn kind(&self) -> &OdbcValueKind {
        &self.kind
    }

    /// Returns whether this value is NULL.
    pub fn is_null(&self) -> bool {
        matches!(self.kind, OdbcValueKind::Null)
    }

    /// Returns this value as a signed integer where possible.
    pub fn as_i64(&self) -> Option<i64> {
        match self.kind {
            OdbcValueKind::TinyInt(value) => Some(i64::from(value)),
            OdbcValueKind::SmallInt(value) => Some(i64::from(value)),
            OdbcValueKind::Integer(value) => Some(i64::from(value)),
            OdbcValueKind::BigInt(value) => Some(value),
            _ => None,
        }
    }

    /// Returns this value as `f64` where possible.
    pub fn as_f64(&self) -> Option<f64> {
        match self.kind {
            OdbcValueKind::Real(value) => Some(f64::from(value)),
            OdbcValueKind::Double(value) => Some(value),
            _ => None,
        }
    }

    /// Returns this value as text where possible.
    pub fn as_str(&self) -> Option<Cow<'_, str>> {
        match &self.kind {
            OdbcValueKind::Text(value) => Some(Cow::Borrowed(value)),
            _ => None,
        }
    }

    /// Returns this value as bytes where possible.
    pub fn as_bytes(&self) -> Option<Cow<'_, [u8]>> {
        match &self.kind {
            OdbcValueKind::Binary(value) => Some(Cow::Borrowed(value)),
            _ => None,
        }
    }
}

impl sqlx_core::value::Value for OdbcValue {
    type Database = crate::Odbc;

    fn as_ref(&self) -> <Self::Database as sqlx_core::database::Database>::ValueRef<'_> {
        OdbcValueRef { value: self }
    }

    fn type_info(&self) -> Cow<'_, crate::OdbcTypeInfo> {
        Cow::Owned(self.kind.type_info())
    }

    fn is_null(&self) -> bool {
        self.is_null()
    }
}

/// Borrowed ODBC value reference.
#[derive(Debug, Clone, Copy)]
pub struct OdbcValueRef<'r> {
    value: &'r OdbcValue,
}

impl<'r> OdbcValueRef<'r> {
    /// Returns this value as a signed integer where possible.
    pub fn as_i64(&self) -> Option<i64> {
        self.value.as_i64()
    }

    /// Returns this value as `f64` where possible.
    pub fn as_f64(&self) -> Option<f64> {
        self.value.as_f64()
    }

    /// Returns this value as borrowed text where possible.
    pub fn as_str(&self) -> Option<&'r str> {
        match &self.value.kind {
            OdbcValueKind::Text(value) => Some(value),
            _ => None,
        }
    }

    /// Returns this value as borrowed bytes where possible.
    pub fn as_bytes(&self) -> Option<&'r [u8]> {
        match &self.value.kind {
            OdbcValueKind::Binary(value) => Some(value),
            _ => None,
        }
    }

    /// Returns this value as a boolean where possible.
    pub fn as_bool(&self) -> Option<bool> {
        match &self.value.kind {
            OdbcValueKind::Bit(value) => Some(*value),
            OdbcValueKind::TinyInt(value) => Some(*value != 0),
            OdbcValueKind::SmallInt(value) => Some(*value != 0),
            OdbcValueKind::Integer(value) => Some(*value != 0),
            OdbcValueKind::BigInt(value) => Some(*value != 0),
            OdbcValueKind::Real(value) => Some(*value != 0.0),
            OdbcValueKind::Double(value) => Some(*value != 0.0),
            OdbcValueKind::Text(value) => parse_bool_text(value),
            _ => None,
        }
    }
}

impl<'r> sqlx_core::value::ValueRef<'r> for OdbcValueRef<'r> {
    type Database = crate::Odbc;

    fn to_owned(&self) -> OdbcValue {
        self.value.clone()
    }

    fn type_info(&self) -> Cow<'_, crate::OdbcTypeInfo> {
        Cow::Owned(self.value.kind.type_info())
    }

    fn is_null(&self) -> bool {
        self.value.is_null()
    }
}

macro_rules! impl_decode_integer {
    ($ty:ty) => {
        impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for $ty {
            fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
                value
                    .as_i64()
                    .and_then(|value| Self::try_from(value).ok())
                    .ok_or_else(|| format!("ODBC: cannot decode {}", stringify!($ty)).into())
            }
        }
    };
}

impl_decode_integer!(i8);
impl_decode_integer!(i16);
impl_decode_integer!(i32);
impl_decode_integer!(i64);
impl_decode_integer!(u8);
impl_decode_integer!(u16);
impl_decode_integer!(u32);
impl_decode_integer!(u64);

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for bool {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        value
            .as_bool()
            .ok_or_else(|| "ODBC: cannot decode bool".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for f32 {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        value
            .as_f64()
            .map(|value| value as f32)
            .ok_or_else(|| "ODBC: cannot decode f32".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for f64 {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        value
            .as_f64()
            .ok_or_else(|| "ODBC: cannot decode f64".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for String {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        if let Some(text) = value.as_str() {
            return Ok(text.to_owned());
        }

        if let Some(bytes) = value.as_bytes() {
            return Ok(String::from_utf8(bytes.to_vec())?);
        }

        Err("ODBC: cannot decode String".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for &'r str {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        if let Some(text) = value.as_str() {
            return Ok(text);
        }

        Err("ODBC: cannot decode &str".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for Vec<u8> {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        value
            .as_bytes()
            .map(<[u8]>::to_vec)
            .ok_or_else(|| "ODBC: cannot decode Vec<u8>".into())
    }
}

impl<'r> sqlx_core::decode::Decode<'r, crate::Odbc> for &'r [u8] {
    fn decode(value: OdbcValueRef<'r>) -> Result<Self, sqlx_core::error::BoxDynError> {
        value
            .as_bytes()
            .ok_or_else(|| "ODBC: cannot decode &[u8]".into())
    }
}

fn parse_bool_text(value: &str) -> Option<bool> {
    match value.trim() {
        "0" | "0.0" | "false" | "FALSE" | "f" | "F" => Some(false),
        "1" | "1.0" | "true" | "TRUE" | "t" | "T" => Some(true),
        value => value
            .parse::<f64>()
            .map(|value| value != 0.0)
            .or_else(|_| value.parse::<i64>().map(|value| value != 0))
            .ok(),
    }
}

/// Supported owned ODBC value kinds.
#[derive(Debug, Clone, PartialEq)]
pub enum OdbcValueKind {
    /// NULL value.
    Null,
    /// 8-bit signed integer.
    TinyInt(i8),
    /// 16-bit signed integer.
    SmallInt(i16),
    /// 32-bit signed integer.
    Integer(i32),
    /// 64-bit signed integer.
    BigInt(i64),
    /// 32-bit float.
    Real(f32),
    /// 64-bit float.
    Double(f64),
    /// Boolean value.
    Bit(bool),
    /// Text value.
    Text(String),
    /// Binary value.
    Binary(Vec<u8>),
    /// Date value.
    Date(odbc_api::sys::Date),
    /// Time value.
    Time(odbc_api::sys::Time),
    /// Timestamp value.
    Timestamp(odbc_api::sys::Timestamp),
}

impl OdbcValueKind {
    fn type_info(&self) -> crate::OdbcTypeInfo {
        let data_type = match self {
            Self::Null => odbc_api::DataType::Unknown,
            Self::TinyInt(_) => odbc_api::DataType::TinyInt,
            Self::SmallInt(_) => odbc_api::DataType::SmallInt,
            Self::Integer(_) => odbc_api::DataType::Integer,
            Self::BigInt(_) => odbc_api::DataType::BigInt,
            Self::Real(_) => odbc_api::DataType::Real,
            Self::Double(_) => odbc_api::DataType::Double,
            Self::Bit(_) => odbc_api::DataType::Bit,
            Self::Text(_) => odbc_api::DataType::WVarchar { length: None },
            Self::Binary(_) => odbc_api::DataType::Varbinary { length: None },
            Self::Date(_) => odbc_api::DataType::Date,
            Self::Time(_) => odbc_api::DataType::Time { precision: 0 },
            Self::Timestamp(_) => odbc_api::DataType::Timestamp { precision: 6 },
        };

        crate::OdbcTypeInfo::new(data_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_values_convert_to_i64() {
        assert_eq!(OdbcValue::new(OdbcValueKind::TinyInt(1)).as_i64(), Some(1));
        assert_eq!(OdbcValue::new(OdbcValueKind::SmallInt(2)).as_i64(), Some(2));
        assert_eq!(OdbcValue::new(OdbcValueKind::Integer(3)).as_i64(), Some(3));
        assert_eq!(OdbcValue::new(OdbcValueKind::BigInt(4)).as_i64(), Some(4));
    }

    #[test]
    fn text_and_bytes_borrow_from_value() {
        let text = OdbcValue::new(OdbcValueKind::Text("hello".to_owned()));
        assert_eq!(text.as_str().as_deref(), Some("hello"));

        let bytes = OdbcValue::new(OdbcValueKind::Binary(vec![1, 2, 3]));
        assert_eq!(bytes.as_bytes().as_deref(), Some(&[1, 2, 3][..]));
    }

    #[test]
    fn null_reports_null() {
        assert!(OdbcValue::new(OdbcValueKind::Null).is_null());
    }

    #[test]
    fn borrowed_values_decode_basic_scalars() {
        use sqlx_core::decode::Decode;
        use sqlx_core::value::Value;

        let int = OdbcValue::new(OdbcValueKind::BigInt(42));
        assert_eq!(
            <i32 as Decode<crate::Odbc>>::decode(int.as_ref()).unwrap(),
            42
        );

        let truthy = OdbcValue::new(OdbcValueKind::Text("true".to_owned()));
        assert!(<bool as Decode<crate::Odbc>>::decode(truthy.as_ref()).unwrap());

        let text = OdbcValue::new(OdbcValueKind::Text("hello".to_owned()));
        assert_eq!(
            <String as Decode<crate::Odbc>>::decode(text.as_ref()).unwrap(),
            "hello"
        );

        let bytes = OdbcValue::new(OdbcValueKind::Binary(vec![1, 2, 3]));
        assert_eq!(
            <Vec<u8> as Decode<crate::Odbc>>::decode(bytes.as_ref()).unwrap(),
            vec![1, 2, 3]
        );
    }
}
