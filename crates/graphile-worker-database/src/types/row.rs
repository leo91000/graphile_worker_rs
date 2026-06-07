use std::collections::HashMap;

use crate::DbError;

use super::{DbCell, FromDbCell};

#[derive(Clone, Debug, Default)]
pub struct DbRow {
    cells: HashMap<String, DbCell>,
}

impl DbRow {
    pub fn new(cells: HashMap<String, DbCell>) -> Self {
        Self { cells }
    }

    pub fn try_get<T: FromDbCell>(&self, name: &str) -> Result<T, DbError> {
        let cell = self.cells.get(name).ok_or_else(|| {
            DbError::new(format!("column `{name}` was not present in query result"))
        })?;
        T::from_cell(name, cell)
    }
}
