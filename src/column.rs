use crate::{Odbc, OdbcTypeInfo};

/// Column metadata for an ODBC row or statement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OdbcColumn {
    ordinal: usize,
    name: String,
    type_info: OdbcTypeInfo,
}

impl OdbcColumn {
    /// Creates column metadata.
    pub fn new(ordinal: usize, name: impl Into<String>, type_info: OdbcTypeInfo) -> Self {
        Self {
            ordinal,
            name: name.into(),
            type_info,
        }
    }
}

impl sqlx_core::column::Column for OdbcColumn {
    type Database = Odbc;

    fn ordinal(&self) -> usize {
        self.ordinal
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn type_info(&self) -> &OdbcTypeInfo {
        &self.type_info
    }
}
