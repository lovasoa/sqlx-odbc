use odbc_api::DataType;
use std::fmt::{Display, Formatter, Result as FmtResult};

/// Type information for an ODBC value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcTypeInfo {
    data_type: DataType,
}

impl OdbcTypeInfo {
    /// Creates type information from an `odbc-api` data type.
    pub const fn new(data_type: DataType) -> Self {
        Self { data_type }
    }

    /// Returns the underlying `odbc-api` data type.
    pub const fn data_type(&self) -> DataType {
        self.data_type
    }

    /// `BIGINT` type information.
    pub const BIGINT: Self = Self::new(DataType::BigInt);

    /// `BIT` type information.
    pub const BIT: Self = Self::new(DataType::Bit);

    /// `DATE` type information.
    pub const DATE: Self = Self::new(DataType::Date);

    /// `DOUBLE` type information.
    pub const DOUBLE: Self = Self::new(DataType::Double);

    /// `INTEGER` type information.
    pub const INTEGER: Self = Self::new(DataType::Integer);

    /// `REAL` type information.
    pub const REAL: Self = Self::new(DataType::Real);

    /// `SMALLINT` type information.
    pub const SMALLINT: Self = Self::new(DataType::SmallInt);

    /// `TINYINT` type information.
    pub const TINYINT: Self = Self::new(DataType::TinyInt);

    /// `UNKNOWN` type information.
    pub const UNKNOWN: Self = Self::new(DataType::Unknown);

    /// `TIME` type information with zero fractional precision.
    pub const TIME: Self = Self::new(DataType::Time { precision: 0 });

    /// `TIMESTAMP` type information with zero fractional precision.
    pub const TIMESTAMP: Self = Self::new(DataType::Timestamp { precision: 0 });

    /// Creates `VARCHAR` type information.
    pub const fn varchar(length: Option<std::num::NonZeroUsize>) -> Self {
        Self::new(DataType::Varchar { length })
    }

    /// Creates `VARBINARY` type information.
    pub const fn varbinary(length: Option<std::num::NonZeroUsize>) -> Self {
        Self::new(DataType::Varbinary { length })
    }

    /// Creates `DECIMAL` type information.
    pub const fn decimal(precision: usize, scale: i16) -> Self {
        Self::new(DataType::Decimal { precision, scale })
    }

    /// Creates `NUMERIC` type information.
    pub const fn numeric(precision: usize, scale: i16) -> Self {
        Self::new(DataType::Numeric { precision, scale })
    }
}

impl sqlx_core::type_info::TypeInfo for OdbcTypeInfo {
    fn is_null(&self) -> bool {
        false
    }

    fn name(&self) -> &str {
        match self.data_type {
            DataType::BigInt => "BIGINT",
            DataType::Binary { .. } => "BINARY",
            DataType::Bit => "BIT",
            DataType::Char { .. } => "CHAR",
            DataType::Date => "DATE",
            DataType::Decimal { .. } => "DECIMAL",
            DataType::Double => "DOUBLE",
            DataType::Float { .. } => "FLOAT",
            DataType::Integer => "INTEGER",
            DataType::LongVarbinary { .. } => "LONGVARBINARY",
            DataType::LongVarchar { .. } => "LONGVARCHAR",
            DataType::Numeric { .. } => "NUMERIC",
            DataType::Real => "REAL",
            DataType::SmallInt => "SMALLINT",
            DataType::Time { .. } => "TIME",
            DataType::Timestamp { .. } => "TIMESTAMP",
            DataType::TinyInt => "TINYINT",
            DataType::Varbinary { .. } => "VARBINARY",
            DataType::Varchar { .. } => "VARCHAR",
            DataType::WChar { .. } => "WCHAR",
            DataType::WLongVarchar { .. } => "WLONGVARCHAR",
            DataType::WVarchar { .. } => "WVARCHAR",
            DataType::Unknown => "UNKNOWN",
            DataType::Other { .. } => "OTHER",
        }
    }
}

/// Helper predicates for `odbc-api` data type groups.
pub trait DataTypeExt {
    /// Returns the canonical display name for this type.
    fn name(self) -> &'static str;

    /// Returns whether this type carries character data.
    fn accepts_character_data(self) -> bool;

    /// Returns whether this type carries binary data.
    fn accepts_binary_data(self) -> bool;

    /// Returns whether this type carries numeric data.
    fn accepts_numeric_data(self) -> bool;
}

impl DataTypeExt for DataType {
    fn name(self) -> &'static str {
        match self {
            DataType::BigInt => "BIGINT",
            DataType::Binary { .. } => "BINARY",
            DataType::Bit => "BIT",
            DataType::Char { .. } => "CHAR",
            DataType::Date => "DATE",
            DataType::Decimal { .. } => "DECIMAL",
            DataType::Double => "DOUBLE",
            DataType::Float { .. } => "FLOAT",
            DataType::Integer => "INTEGER",
            DataType::LongVarbinary { .. } => "LONGVARBINARY",
            DataType::LongVarchar { .. } => "LONGVARCHAR",
            DataType::Numeric { .. } => "NUMERIC",
            DataType::Real => "REAL",
            DataType::SmallInt => "SMALLINT",
            DataType::Time { .. } => "TIME",
            DataType::Timestamp { .. } => "TIMESTAMP",
            DataType::TinyInt => "TINYINT",
            DataType::Varbinary { .. } => "VARBINARY",
            DataType::Varchar { .. } => "VARCHAR",
            DataType::WChar { .. } => "WCHAR",
            DataType::WLongVarchar { .. } => "WLONGVARCHAR",
            DataType::WVarchar { .. } => "WVARCHAR",
            DataType::Unknown => "UNKNOWN",
            DataType::Other { .. } => "OTHER",
        }
    }

    fn accepts_character_data(self) -> bool {
        matches!(
            self,
            DataType::Char { .. }
                | DataType::Varchar { .. }
                | DataType::LongVarchar { .. }
                | DataType::WChar { .. }
                | DataType::WVarchar { .. }
                | DataType::WLongVarchar { .. }
        )
    }

    fn accepts_binary_data(self) -> bool {
        matches!(
            self,
            DataType::Binary { .. } | DataType::Varbinary { .. } | DataType::LongVarbinary { .. }
        )
    }

    fn accepts_numeric_data(self) -> bool {
        matches!(
            self,
            DataType::TinyInt
                | DataType::SmallInt
                | DataType::Integer
                | DataType::BigInt
                | DataType::Real
                | DataType::Float { .. }
                | DataType::Double
                | DataType::Decimal { .. }
                | DataType::Numeric { .. }
        )
    }
}

impl Display for OdbcTypeInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.write_str(sqlx_core::type_info::TypeInfo::name(self))
    }
}
