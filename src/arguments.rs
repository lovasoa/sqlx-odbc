use crate::DataTypeExt;
use odbc_api::{
    parameter::{InputParameter, VarBinaryBox, VarCharBox, VarWCharBox, WithDataType},
    IntoParameter, Nullable,
};

/// Values that can currently be bound to ODBC parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum OdbcArgumentValue {
    /// UTF-8 text parameter.
    Text(String),
    /// Binary parameter.
    Bytes(Vec<u8>),
    /// Signed integer parameter.
    Int(i64),
    /// Unsigned integer parameter.
    UInt(u64),
    /// Boolean parameter.
    Bit(bool),
    /// Floating point parameter.
    Float(f64),
    /// Date parameter.
    Date(odbc_api::sys::Date),
    /// Time parameter.
    Time(odbc_api::sys::Time),
    /// Timestamp parameter.
    Timestamp(odbc_api::sys::Timestamp),
    /// Typed NULL parameter.
    Null(crate::OdbcTypeInfo),
}

/// Argument buffer for ODBC queries.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct OdbcArguments {
    values: Vec<OdbcArgumentValue>,
}

/// Owned ODBC parameter storage ready to bind with `odbc-api`.
///
/// `odbc-api` implements `ParameterCollectionRef` for `&[Box<dyn InputParameter>]`, so executor
/// code can pass `collection.as_slice()` to `Connection::execute` or `Preallocated::execute`.
#[derive(Default)]
pub struct OdbcParameterCollection {
    parameters: Vec<Box<dyn InputParameter>>,
}

impl std::fmt::Debug for OdbcParameterCollection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OdbcParameterCollection")
            .field("len", &self.parameters.len())
            .finish()
    }
}

impl OdbcParameterCollection {
    /// Converts raw SQLx ODBC argument values into owned `odbc-api` input parameters.
    pub fn from_values(values: &[OdbcArgumentValue]) -> Self {
        let parameters = values.iter().map(value_to_parameter).collect();

        Self { parameters }
    }

    /// Returns the number of parameters.
    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    /// Returns `true` when no parameters are present.
    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }

    /// Returns the parameter slice accepted by `odbc-api` execution methods.
    pub fn as_slice(&self) -> &[Box<dyn InputParameter>] {
        &self.parameters
    }
}

impl OdbcArguments {
    /// Adds a raw ODBC argument value.
    pub fn add_value(&mut self, value: OdbcArgumentValue) {
        self.values.push(value);
    }

    /// Returns the number of arguments.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns `true` when no arguments have been added.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns the raw argument values.
    pub fn values(&self) -> &[OdbcArgumentValue] {
        &self.values
    }

    /// Converts these arguments into owned `odbc-api` parameters.
    pub fn to_odbc_parameter_collection(&self) -> OdbcParameterCollection {
        OdbcParameterCollection::from_values(&self.values)
    }
}

impl sqlx_core::arguments::Arguments for OdbcArguments {
    type Database = crate::Odbc;

    fn reserve(&mut self, additional: usize, _size: usize) {
        self.values.reserve(additional);
    }

    fn add<'t, T>(&mut self, value: T) -> Result<(), sqlx_core::error::BoxDynError>
    where
        T: sqlx_core::encode::Encode<'t, Self::Database> + sqlx_core::types::Type<Self::Database>,
    {
        let _ = value.encode(&mut self.values)?;
        Ok(())
    }

    fn len(&self) -> usize {
        self.values.len()
    }
}

sqlx_core::impl_into_arguments_for_arguments!(OdbcArguments);

impl<'q, T> sqlx_core::encode::Encode<'q, crate::Odbc> for Option<T>
where
    T: sqlx_core::encode::Encode<'q, crate::Odbc> + sqlx_core::types::Type<crate::Odbc> + 'q,
{
    fn encode(
        self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        match self {
            Some(value) => value.encode(buf),
            None => {
                buf.push(OdbcArgumentValue::Null(T::type_info()));
                Ok(sqlx_core::encode::IsNull::Yes)
            }
        }
    }

    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        match self {
            Some(value) => value.encode_by_ref(buf),
            None => {
                buf.push(OdbcArgumentValue::Null(T::type_info()));
                Ok(sqlx_core::encode::IsNull::Yes)
            }
        }
    }

    fn produces(&self) -> Option<crate::OdbcTypeInfo> {
        match self {
            Some(value) => value.produces(),
            None => Some(T::type_info()),
        }
    }
}

macro_rules! impl_integer {
    ($ty:ty, $type_info:expr, $($compatible:pat_param)|+ $(,)?) => {
        impl sqlx_core::types::Type<crate::Odbc> for $ty {
            fn type_info() -> crate::OdbcTypeInfo {
                crate::OdbcTypeInfo::new($type_info)
            }

            fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
                matches!(
                    ty.data_type(),
                    $($compatible)|+
                        | odbc_api::DataType::Numeric { .. }
                        | odbc_api::DataType::Decimal { .. }
                ) || ty.data_type().accepts_character_data()
            }
        }

        impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for $ty {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<OdbcArgumentValue>,
            ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
                buf.push(OdbcArgumentValue::Int(i64::from(*self)));
                Ok(sqlx_core::encode::IsNull::No)
            }
        }
    };
}

impl_integer!(
    i8,
    odbc_api::DataType::TinyInt,
    odbc_api::DataType::TinyInt
        | odbc_api::DataType::SmallInt
        | odbc_api::DataType::Integer
        | odbc_api::DataType::BigInt,
);
impl_integer!(
    i16,
    odbc_api::DataType::SmallInt,
    odbc_api::DataType::TinyInt
        | odbc_api::DataType::SmallInt
        | odbc_api::DataType::Integer
        | odbc_api::DataType::BigInt,
);
impl_integer!(
    i32,
    odbc_api::DataType::Integer,
    odbc_api::DataType::TinyInt
        | odbc_api::DataType::SmallInt
        | odbc_api::DataType::Integer
        | odbc_api::DataType::BigInt,
);
impl_integer!(
    i64,
    odbc_api::DataType::BigInt,
    odbc_api::DataType::TinyInt
        | odbc_api::DataType::SmallInt
        | odbc_api::DataType::Integer
        | odbc_api::DataType::BigInt,
);

macro_rules! impl_unsigned {
    ($ty:ty, $type_info:expr, $($compatible:pat_param)|+ $(,)?) => {
        impl sqlx_core::types::Type<crate::Odbc> for $ty {
            fn type_info() -> crate::OdbcTypeInfo {
                crate::OdbcTypeInfo::new($type_info)
            }

            fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
                matches!(
                    ty.data_type(),
                    $($compatible)|+
                        | odbc_api::DataType::Numeric { .. }
                        | odbc_api::DataType::Decimal { .. }
                ) || ty.data_type().accepts_character_data()
            }
        }

        impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for $ty {
            fn encode_by_ref(
                &self,
                buf: &mut Vec<OdbcArgumentValue>,
            ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
                buf.push(OdbcArgumentValue::Int(i64::from(*self)));
                Ok(sqlx_core::encode::IsNull::No)
            }
        }
    };
}

impl_unsigned!(
    u8,
    odbc_api::DataType::TinyInt,
    odbc_api::DataType::TinyInt
        | odbc_api::DataType::SmallInt
        | odbc_api::DataType::Integer
        | odbc_api::DataType::BigInt,
);
impl_unsigned!(
    u16,
    odbc_api::DataType::SmallInt,
    odbc_api::DataType::SmallInt | odbc_api::DataType::Integer | odbc_api::DataType::BigInt,
);
impl_unsigned!(
    u32,
    odbc_api::DataType::Integer,
    odbc_api::DataType::Integer | odbc_api::DataType::BigInt,
);

impl sqlx_core::types::Type<crate::Odbc> for u64 {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::BIGINT
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        matches!(
            ty.data_type(),
            odbc_api::DataType::Integer
                | odbc_api::DataType::BigInt
                | odbc_api::DataType::Numeric { .. }
                | odbc_api::DataType::Decimal { .. }
        ) || ty.data_type().accepts_character_data()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for u64 {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        if let Ok(value) = i64::try_from(*self) {
            buf.push(OdbcArgumentValue::Int(value));
        } else {
            buf.push(OdbcArgumentValue::UInt(*self));
        }

        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl sqlx_core::types::Type<crate::Odbc> for bool {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::new(odbc_api::DataType::Bit)
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        ty.data_type().accepts_numeric_data() || ty.data_type().accepts_character_data()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for bool {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Bit(*self));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl sqlx_core::types::Type<crate::Odbc> for f32 {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::new(odbc_api::DataType::Real)
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        ty.data_type().accepts_numeric_data() || ty.data_type().accepts_character_data()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for f32 {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Float(f64::from(*self)));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl sqlx_core::types::Type<crate::Odbc> for f64 {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::new(odbc_api::DataType::Double)
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        ty.data_type().accepts_numeric_data() || ty.data_type().accepts_character_data()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for f64 {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Float(*self));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl sqlx_core::types::Type<crate::Odbc> for str {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::new(odbc_api::DataType::WVarchar { length: None })
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        ty.data_type().accepts_character_data()
    }
}

impl sqlx_core::types::Type<crate::Odbc> for String {
    fn type_info() -> crate::OdbcTypeInfo {
        <str as sqlx_core::types::Type<crate::Odbc>>::type_info()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for &'q str {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Text((*self).to_owned()));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for String {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Text(self.clone()));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl sqlx_core::types::Type<crate::Odbc> for [u8] {
    fn type_info() -> crate::OdbcTypeInfo {
        crate::OdbcTypeInfo::new(odbc_api::DataType::Varbinary { length: None })
    }

    fn compatible(ty: &crate::OdbcTypeInfo) -> bool {
        ty.data_type().accepts_binary_data()
    }
}

impl sqlx_core::types::Type<crate::Odbc> for Vec<u8> {
    fn type_info() -> crate::OdbcTypeInfo {
        <[u8] as sqlx_core::types::Type<crate::Odbc>>::type_info()
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for &'q [u8] {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Bytes((*self).to_owned()));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

impl<'q> sqlx_core::encode::Encode<'q, crate::Odbc> for Vec<u8> {
    fn encode_by_ref(
        &self,
        buf: &mut Vec<OdbcArgumentValue>,
    ) -> Result<sqlx_core::encode::IsNull, sqlx_core::error::BoxDynError> {
        buf.push(OdbcArgumentValue::Bytes(self.clone()));
        Ok(sqlx_core::encode::IsNull::No)
    }
}

fn value_to_parameter(value: &OdbcArgumentValue) -> Box<dyn InputParameter> {
    match value {
        OdbcArgumentValue::Text(value) => Box::new(value.clone().into_parameter()),
        OdbcArgumentValue::Bytes(value) => Box::new(value.clone().into_parameter()),
        OdbcArgumentValue::Int(value) => Box::new(Some(*value).into_parameter()),
        OdbcArgumentValue::UInt(value) => Box::new(
            WithDataType::new(Nullable::new(*value), odbc_api::DataType::BigInt).into_parameter(),
        ),
        OdbcArgumentValue::Bit(value) => Box::new(odbc_api::Bit::from_bool(*value)),
        OdbcArgumentValue::Float(value) => Box::new(Some(*value).into_parameter()),
        OdbcArgumentValue::Date(value) => Box::new(Nullable::new(*value).into_parameter()),
        OdbcArgumentValue::Time(value) => Box::new(
            WithDataType::new(
                Nullable::new(*value),
                odbc_api::DataType::Time { precision: 0 },
            )
            .into_parameter(),
        ),
        OdbcArgumentValue::Timestamp(value) => Box::new(
            WithDataType::new(
                Nullable::new(*value),
                odbc_api::DataType::Timestamp { precision: 9 },
            )
            .into_parameter(),
        ),
        OdbcArgumentValue::Null(type_info) => null_parameter(type_info.data_type()),
    }
}

fn null_parameter(data_type: odbc_api::DataType) -> Box<dyn InputParameter> {
    match data_type {
        odbc_api::DataType::TinyInt => Box::new(Nullable::<i8>::null()),
        odbc_api::DataType::SmallInt => Box::new(Nullable::<i16>::null()),
        odbc_api::DataType::Integer => Box::new(Nullable::<i32>::null()),
        odbc_api::DataType::BigInt => Box::new(Nullable::<i64>::null()),
        odbc_api::DataType::Bit => Box::new(Nullable::<odbc_api::Bit>::null()),
        odbc_api::DataType::Real => Box::new(Nullable::<f32>::null()),
        odbc_api::DataType::Double => Box::new(Nullable::<f64>::null()),
        odbc_api::DataType::Float { .. } => {
            Box::new(WithDataType::new(Nullable::<f64>::null(), data_type))
        }
        odbc_api::DataType::Date => Box::new(Nullable::<odbc_api::sys::Date>::null()),
        odbc_api::DataType::Time { .. } => Box::new(WithDataType::new(
            Nullable::<odbc_api::sys::Time>::null(),
            data_type,
        )),
        odbc_api::DataType::Timestamp { .. } => Box::new(WithDataType::new(
            Nullable::<odbc_api::sys::Timestamp>::null(),
            data_type,
        )),
        odbc_api::DataType::Varbinary { .. }
        | odbc_api::DataType::LongVarbinary { .. }
        | odbc_api::DataType::Binary { .. } => {
            Box::new(WithDataType::new(VarBinaryBox::null(), data_type))
        }
        odbc_api::DataType::WVarchar { .. }
        | odbc_api::DataType::WLongVarchar { .. }
        | odbc_api::DataType::WChar { .. } => {
            Box::new(WithDataType::new(VarWCharBox::null(), data_type))
        }
        odbc_api::DataType::Char { .. }
        | odbc_api::DataType::Varchar { .. }
        | odbc_api::DataType::LongVarchar { .. }
        | odbc_api::DataType::Numeric { .. }
        | odbc_api::DataType::Decimal { .. }
        | odbc_api::DataType::Unknown
        | odbc_api::DataType::Other { .. } => {
            Box::new(WithDataType::new(VarCharBox::null(), data_type))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use odbc_api::{
        handles::{CData, HasDataType},
        ParameterCollectionRef,
    };

    #[test]
    fn argument_buffer_tracks_values_in_order() {
        let mut arguments = OdbcArguments::default();

        arguments.add_value(OdbcArgumentValue::Int(7));
        arguments.add_value(OdbcArgumentValue::Text("abc".to_owned()));
        arguments.add_value(OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(
            odbc_api::DataType::Integer,
        )));

        assert_eq!(arguments.len(), 3);
        assert_eq!(
            arguments.values(),
            &[
                OdbcArgumentValue::Int(7),
                OdbcArgumentValue::Text("abc".to_owned()),
                OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(odbc_api::DataType::Integer))
            ]
        );
    }

    #[test]
    fn sqlx_arguments_add_encodes_basic_scalars() {
        let mut arguments = OdbcArguments::default();

        sqlx_core::arguments::Arguments::add(&mut arguments, 7_i32).unwrap();
        sqlx_core::arguments::Arguments::add(&mut arguments, "abc").unwrap();
        sqlx_core::arguments::Arguments::add(&mut arguments, vec![1_u8, 2, 3]).unwrap();

        assert_eq!(
            arguments.values(),
            &[
                OdbcArgumentValue::Int(7),
                OdbcArgumentValue::Text("abc".to_owned()),
                OdbcArgumentValue::Bytes(vec![1, 2, 3])
            ]
        );
    }

    #[test]
    fn sqlx_arguments_add_encodes_typed_null_option() {
        let mut arguments = OdbcArguments::default();

        sqlx_core::arguments::Arguments::add(&mut arguments, Option::<i32>::None).unwrap();

        assert_eq!(
            arguments.values(),
            &[OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(
                odbc_api::DataType::Integer
            ))]
        );

        let collection = arguments.to_odbc_parameter_collection();
        assert_eq!(
            collection.as_slice()[0].data_type(),
            odbc_api::DataType::Integer
        );
    }

    #[test]
    fn sqlx_arguments_reserve_and_len_work() {
        let mut arguments = OdbcArguments::default();

        sqlx_core::arguments::Arguments::reserve(&mut arguments, 2, 16);
        sqlx_core::arguments::Arguments::add(&mut arguments, true).unwrap();
        sqlx_core::arguments::Arguments::add(&mut arguments, 1.5_f64).unwrap();

        assert_eq!(sqlx_core::arguments::Arguments::len(&arguments), 2);
        assert_eq!(
            arguments.values(),
            &[OdbcArgumentValue::Bit(true), OdbcArgumentValue::Float(1.5)]
        );
    }

    #[test]
    fn parameter_collection_converts_basic_values_to_odbc_parameters() {
        let values = [
            OdbcArgumentValue::Text("abc".to_owned()),
            OdbcArgumentValue::Bytes(vec![1, 2, 3]),
            OdbcArgumentValue::Int(7),
            OdbcArgumentValue::UInt(8),
            OdbcArgumentValue::Bit(true),
            OdbcArgumentValue::Float(1.5),
        ];

        let collection = OdbcParameterCollection::from_values(&values);

        assert_eq!(collection.len(), values.len());
        assert!(matches!(
            collection.as_slice()[0].data_type(),
            odbc_api::DataType::Varchar { .. }
                | odbc_api::DataType::WVarchar { .. }
                | odbc_api::DataType::WLongVarchar { .. }
        ));
        assert!(matches!(
            collection.as_slice()[1].data_type(),
            odbc_api::DataType::Varbinary { .. }
        ));
        assert_eq!(
            collection.as_slice()[2].data_type(),
            odbc_api::DataType::BigInt
        );
        assert_eq!(
            collection.as_slice()[3].data_type(),
            odbc_api::DataType::BigInt
        );
        assert_eq!(
            collection.as_slice()[4].data_type(),
            odbc_api::DataType::Bit
        );
        assert_eq!(
            collection.as_slice()[5].data_type(),
            odbc_api::DataType::Double
        );
    }

    #[test]
    fn parameter_collection_converts_temporal_values_to_typed_odbc_parameters() {
        let values = [
            OdbcArgumentValue::Date(odbc_api::sys::Date {
                year: 2026,
                month: 5,
                day: 29,
            }),
            OdbcArgumentValue::Time(odbc_api::sys::Time {
                hour: 12,
                minute: 30,
                second: 45,
            }),
            OdbcArgumentValue::Timestamp(odbc_api::sys::Timestamp {
                year: 2026,
                month: 5,
                day: 29,
                hour: 12,
                minute: 30,
                second: 45,
                fraction: 123_456_789,
            }),
        ];

        let collection = OdbcParameterCollection::from_values(&values);

        assert_eq!(
            collection.as_slice()[0].data_type(),
            odbc_api::DataType::Date
        );
        assert_eq!(
            collection.as_slice()[1].data_type(),
            odbc_api::DataType::Time { precision: 0 }
        );
        assert_eq!(
            collection.as_slice()[2].data_type(),
            odbc_api::DataType::Timestamp { precision: 9 }
        );
    }

    #[test]
    fn parameter_collection_converts_typed_nulls_to_requested_data_types() {
        let values = [
            OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(odbc_api::DataType::Integer)),
            OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(odbc_api::DataType::WVarchar {
                length: None,
            })),
            OdbcArgumentValue::Null(crate::OdbcTypeInfo::new(odbc_api::DataType::Decimal {
                precision: 10,
                scale: 2,
            })),
        ];

        let collection = OdbcParameterCollection::from_values(&values);

        assert_eq!(
            collection.as_slice()[0].data_type(),
            odbc_api::DataType::Integer
        );
        assert_eq!(
            collection.as_slice()[1].data_type(),
            odbc_api::DataType::WVarchar { length: None }
        );
        assert_eq!(
            collection.as_slice()[2].data_type(),
            odbc_api::DataType::Decimal {
                precision: 10,
                scale: 2
            }
        );
    }

    #[test]
    fn parameter_collection_slice_matches_odbc_api_binding_shape() {
        fn assert_parameter_collection_ref<T: ParameterCollectionRef>(_parameters: T) {}

        let mut arguments = OdbcArguments::default();
        sqlx_core::arguments::Arguments::add(&mut arguments, "abc").unwrap();
        let collection = arguments.to_odbc_parameter_collection();

        assert_parameter_collection_ref(collection.as_slice());
    }

    #[test]
    fn fixed_sized_parameter_uses_explicit_non_null_indicator() {
        let mut arguments = OdbcArguments::default();

        sqlx_core::arguments::Arguments::add(&mut arguments, 5_i32).unwrap();

        let collection = arguments.to_odbc_parameter_collection();
        assert_eq!(collection.len(), 1);
        assert!(!collection.as_slice()[0].indicator_ptr().is_null());
    }
}
