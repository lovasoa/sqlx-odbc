use crate::{Odbc, OdbcColumn, OdbcValue};
use std::sync::Arc;

/// Minimal ODBC row container used by the SQLx-core skeleton.
#[derive(Debug, Clone, Default)]
pub struct OdbcRow {
    columns: Arc<[OdbcColumn]>,
    values: Vec<OdbcValue>,
}

impl OdbcRow {
    /// Creates a row from column metadata and values.
    pub fn new(columns: Vec<OdbcColumn>, values: Vec<OdbcValue>) -> Self {
        Self::new_shared(columns.into(), values)
    }

    pub(crate) fn new_shared(columns: Arc<[OdbcColumn]>, values: Vec<OdbcValue>) -> Self {
        Self { columns, values }
    }
}

impl sqlx_core::row::Row for OdbcRow {
    type Database = Odbc;

    fn columns(&self) -> &[OdbcColumn] {
        self.columns.as_ref()
    }

    fn try_get_raw<I>(
        &self,
        index: I,
    ) -> Result<<Self::Database as sqlx_core::database::Database>::ValueRef<'_>, sqlx_core::Error>
    where
        I: sqlx_core::column::ColumnIndex<Self>,
    {
        let index = index.index(self)?;
        let value = self
            .values
            .get(index)
            .ok_or(sqlx_core::Error::ColumnIndexOutOfBounds {
                index,
                len: self.values.len(),
            })?;

        Ok(sqlx_core::value::Value::as_ref(value))
    }
}

impl sqlx_core::column::ColumnIndex<OdbcRow> for usize {
    fn index(&self, row: &OdbcRow) -> Result<usize, sqlx_core::Error> {
        if *self >= row.columns.len() {
            return Err(sqlx_core::Error::ColumnIndexOutOfBounds {
                index: *self,
                len: row.columns.len(),
            });
        }

        Ok(*self)
    }
}

impl sqlx_core::column::ColumnIndex<OdbcRow> for &str {
    fn index(&self, row: &OdbcRow) -> Result<usize, sqlx_core::Error> {
        if let Some(index) = row
            .columns
            .iter()
            .position(|column| sqlx_core::column::Column::name(column) == *self)
        {
            return Ok(index);
        }

        row.columns
            .iter()
            .position(|column| sqlx_core::column::Column::name(column).eq_ignore_ascii_case(self))
            .ok_or_else(|| sqlx_core::Error::ColumnNotFound((*self).to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OdbcTypeInfo, OdbcValueKind};

    fn create_test_row() -> OdbcRow {
        OdbcRow::new(
            vec![
                OdbcColumn::new(
                    0,
                    "lowercase_col",
                    OdbcTypeInfo::new(odbc_api::DataType::Integer),
                ),
                OdbcColumn::new(
                    1,
                    "UPPERCASE_COL",
                    OdbcTypeInfo::new(odbc_api::DataType::Varchar { length: None }),
                ),
                OdbcColumn::new(
                    2,
                    "MixedCase_Col",
                    OdbcTypeInfo::new(odbc_api::DataType::Double),
                ),
            ],
            vec![
                OdbcValue::new(OdbcValueKind::Integer(42)),
                OdbcValue::new(OdbcValueKind::Text("test".to_owned())),
                OdbcValue::new(OdbcValueKind::Double(std::f64::consts::PI)),
            ],
        )
    }

    #[test]
    fn exact_column_match_works() {
        let row = create_test_row();

        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"lowercase_col", &row).unwrap(),
            0
        );
        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"UPPERCASE_COL", &row).unwrap(),
            1
        );
        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"MixedCase_Col", &row).unwrap(),
            2
        );
    }

    #[test]
    fn case_insensitive_column_match_works() {
        let row = create_test_row();

        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"LOWERCASE_COL", &row).unwrap(),
            0
        );
        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"uppercase_col", &row).unwrap(),
            1
        );
        assert_eq!(
            sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"mixedcase_col", &row).unwrap(),
            2
        );
    }

    #[test]
    fn missing_column_reports_name() {
        let row = create_test_row();
        let error = sqlx_core::column::ColumnIndex::<OdbcRow>::index(&"missing", &row).unwrap_err();

        assert!(matches!(error, sqlx_core::Error::ColumnNotFound(name) if name == "missing"));
    }

    #[test]
    fn try_get_raw_uses_case_insensitive_column_lookup() {
        use sqlx_core::row::Row;

        let row = create_test_row();
        let value = row.try_get_raw("mixedcase_col").unwrap();

        assert_eq!(value.as_f64(), Some(std::f64::consts::PI));
    }

    #[test]
    fn columns_returns_metadata_in_order() {
        use sqlx_core::column::Column;
        use sqlx_core::row::Row;

        let row = create_test_row();
        let columns = row.columns();

        assert_eq!(columns.len(), 3);
        assert_eq!(columns[0].ordinal(), 0);
        assert_eq!(columns[0].name(), "lowercase_col");
        assert_eq!(columns[1].ordinal(), 1);
        assert_eq!(columns[1].name(), "UPPERCASE_COL");
        assert_eq!(columns[2].ordinal(), 2);
        assert_eq!(columns[2].name(), "MixedCase_Col");
    }
}
